use std::ffi::OsStr;

#[derive(PartialEq, Eq, Debug)]
enum GpuDriver {
    AmdGpu, // Modern AMD
    Radeon, // Legacy AMD
    Nouveau, // Modern Open-Source NVIDIA
    Nvidia, // Legacy Proprietary NVIDIA
    I915, // Legacy Intel
    Xe, // Modern Intel
    Other,
}

impl GpuDriver {
    pub fn new(name: Option<&OsStr>) -> Self {
        let Some(name) = name else {
            return Self::Other;
        };

        match name.as_encoded_bytes() {
            b"amdgpu" => Self::AmdGpu,
            b"radeon" => Self::Radeon,
            b"nouveau" => Self::Nouveau,
            b"nvidia" => Self::Nvidia,
            b"i915" => Self::I915,
            b"xe" => Self::Xe,
            _ => {
                log::warn!("Unknown graphics driver name: {}", name.to_string_lossy());
                Self::Other
            }
        }
    }
}

struct GpuDevice {
    render: udev::Device,
    platform: udev::Device,
    driver: GpuDriver,
}

pub fn use_discrete_gpu(command: &mut command::PandoraCommand) -> std::io::Result<()> {
    let Some(best_gpu) = determine_best_gpu()? else {
        return Ok(());
    };

    log::info!("Best GPU device: {:?} ({:?})", best_gpu.render.devpath(), best_gpu.driver);

    if best_gpu.driver == GpuDriver::Nvidia {
        command.env("__NV_PRIME_RENDER_OFFLOAD", "1");
        command.env("__GLX_VENDOR_LIBRARY_NAME", "nvidia");
        command.env("__VK_LAYER_NV_optimus", "NVIDIA_only");
    } else if let Some(id) = best_gpu.render.property_value("ID_PATH_TAG") {
        log::info!("Setting DRI_PRIME to {:?}", id);
        command.env("DRI_PRIME", id.to_os_string());
    }

    let vk_driver = match best_gpu.driver {
        GpuDriver::AmdGpu | GpuDriver::Radeon => Some("*radeon*"),
        GpuDriver::Nouveau => None,
        GpuDriver::Nvidia => Some("*nvidia*"),
        GpuDriver::I915 | GpuDriver::Xe => Some("*intel*"),
        GpuDriver::Other => None,
    };
    if let Some(vk_driver) = vk_driver {
        command.env("VK_LOADER_DRIVERS_SELECT", vk_driver);
    }

    Ok(())
}

fn determine_best_gpu() -> std::io::Result<Option<GpuDevice>> {
    let mut enumerator = udev::Enumerator::new()?;
    enumerator.match_subsystem("drm")?;
    let devices = enumerator.scan_devices()?;

    let mut gpus = Vec::new();

    for device in devices {
        let Some(path) = device.devnode() else {
            continue;
        };
        let Ok(filename) = path.strip_prefix("/dev/dri/") else {
            continue;
        };
        if !filename.as_os_str().as_encoded_bytes().starts_with(b"render") {
            continue;
        }
        let Some(parent) = device.parent() else {
            continue;
        };
        let driver = GpuDriver::new(parent.driver());

        gpus.push(GpuDevice {
            render: device,
            platform: parent,
            driver,
        });
    }

    if gpus.is_empty() {
        return Ok(None);
    } else if gpus.len() == 1 {
        return Ok(Some(gpus.remove(0)));
    }

    let best_device = gpus.into_iter()
        .map(|device| (determine_gpu_priority(&device), device))
        .max_by_key(|(prio, _)| *prio)
        .map(|(_, device)| device);

    Ok(best_device)
}

static SWITCHEROO_DISCRETE_GPU_TAG: once_cell::sync::Lazy<std::ffi::CString> = once_cell::sync::Lazy::new(|| {
    std::ffi::CString::new("switcheroo-discrete-gpu").unwrap()
});

fn determine_gpu_priority(device: &GpuDevice) -> u32 {
    let mut priority = 0;

    match is_discrete_gpu(device) {
        Ok(discrete) => if discrete {
            priority += 2;
        },
        Err(err) => {
            log::error!("Error while checking whether gpu is discrete: {:?}", err);
        },
    }
    if is_default_gpu(device) {
        priority += 1;
    }

    priority
}

fn is_default_gpu(device: &GpuDevice) -> bool {
    device.platform.attribute_value("boot_vga")
        .map(|s| s.as_encoded_bytes() == b"1")
        .unwrap_or(false)
}

fn is_discrete_gpu(device: &GpuDevice) -> std::io::Result<bool> {
    let has_switcheroo_discrete_tag = unsafe {
        use udev::AsRawWithContext;
        udev::ffi::udev_device_has_tag(device.render.as_raw(), SWITCHEROO_DISCRETE_GPU_TAG.as_ptr()) != 0
    };
    if has_switcheroo_discrete_tag {
        return Ok(true);
    }

    match device.driver {
        GpuDriver::AmdGpu => is_discrete_gpu_amdgpu(device),
        GpuDriver::Nouveau => is_discrete_gpu_nouveau(device),
        GpuDriver::Nvidia => Ok(true),
        GpuDriver::I915 => Ok(!device.render.devpath().as_encoded_bytes().starts_with(b"/devices/pci0000:00/0000:00:02.0/drm/")),
        GpuDriver::Xe => is_discrete_gpu_xe(device),
        GpuDriver::Other | GpuDriver::Radeon => Ok(false),
    }
}

const DRM_IOCTL_BASE: char = 'd';
const DRM_COMMAND_BASE: u32 = 0x40;

fn drm_command_read_write<T>(fd: i32, drm_command_index: u32, data: &mut T) -> std::io::Result<()> {
    let request = libc::_IOWR::<T>(DRM_IOCTL_BASE as u32, DRM_COMMAND_BASE + drm_command_index);
    let res = unsafe { libc::ioctl(fd, request, data) };
    if res == 0 {
        return Ok(());
    } else {
        return Err(std::io::Error::last_os_error());
    }
}

fn drm_command_write<T>(fd: i32, drm_command_index: u32, data: &T) -> std::io::Result<()> {
    let request = libc::_IOW::<T>(DRM_IOCTL_BASE as u32, DRM_COMMAND_BASE + drm_command_index);
    let res = unsafe { libc::ioctl(fd, request, data) };
    if res == 0 {
        return Ok(());
    } else {
        return Err(std::io::Error::last_os_error());
    }
}

fn is_discrete_gpu_amdgpu(device: &GpuDevice) -> std::io::Result<bool> {
    let Some(node) = device.render.devnode() else {
        return Ok(false);
    };
    let file = std::fs::OpenOptions::new().read(true).write(true).open(node)?;
    use std::os::fd::AsRawFd;
    let fd = file.as_raw_fd();

    #[repr(C)]
    #[derive(Debug)]
    struct DrmAmdgpuInfoDevice {
        _unused: [u32; 34],
    	ids_flags: u64,
    }
    let mut result = DrmAmdgpuInfoDevice {
        _unused: [0; 34],
        ids_flags: 0,
    };

    #[repr(C)]
    struct DrmAmdgpuInfo {
        return_pointer: u64,
        return_size: u32,
        query: u32,
        _pad: [u32; 16]
    }

    const AMDGPU_INFO_DEV_INFO: u32 = 0x16;
    let mut query = DrmAmdgpuInfo {
        return_pointer: (&mut result as *mut DrmAmdgpuInfoDevice).addr() as u64,
        return_size: std::mem::size_of_val(&result) as u32,
        query: AMDGPU_INFO_DEV_INFO,
        _pad: Default::default(),
    };

    const DRM_AMDGPU_INFO: u32 = 0x05;
    drm_command_write(fd, DRM_AMDGPU_INFO, &mut query)?;

    const AMDGPU_IDS_FLAGS_FUSION: u64 = 0x1;
    Ok((result.ids_flags & AMDGPU_IDS_FLAGS_FUSION) == 0)
}

fn is_discrete_gpu_nouveau(device: &GpuDevice) -> std::io::Result<bool> {
    let Some(node) = device.render.devnode() else {
        return Ok(false);
    };
    let file = std::fs::OpenOptions::new().read(true).write(true).open(node)?;
    use std::os::fd::AsRawFd;
    let fd = file.as_raw_fd();

    #[repr(C)]
    struct NouveauObject {
        parent: *mut NouveauObject,
        handle: u64,
        oclass: u32,
        length: u32,
        data: *mut ()
    }
    let mut nouveau_object = NouveauObject {
        parent: std::ptr::null_mut(),
        handle: 0,
        oclass: 0,
        length: 0,
        data: std::ptr::null_mut(),
    };

    // Init device
    #[repr(C)]
    struct NvifIoctlV0 {
        version: u8,
        r#type: u8,
        pad02: [u8; 4],
        owner: u8,
        route: u8,
        token: u64,
        object: u64,
    }
    #[repr(C)]
    struct NvifIoctlNewV0 {
        version: u8,
        pad01: [u8; 6],
        route: u8,
        token: u64,
        object: u64,
        handle: u32,
        oclass: i32,
    }
    #[repr(C)]
    struct NvDeviceV0 {
        version: u8,
        pad01: [u8; 7],
        device: u64,
    }
    #[repr(C)]
    struct InitArgs {
        ioctl: NvifIoctlV0,
        new: NvifIoctlNewV0,
        dev: NvDeviceV0
    }
    const NVIF_IOCTL_V0_NEW: u8 = 0x02;
    const NVIF_IOCTL_V0_OWNER_ANY: u8 = 0xff;
    const NV_DEVICE: i32 = 0x00000080;
    const NVIF_IOCTL_V0_ROUTE_NVIF: u8 = 0x00;
    let init_args = InitArgs {
        ioctl: NvifIoctlV0 {
            version: 0,
            r#type: NVIF_IOCTL_V0_NEW,
            pad02: Default::default(),
            owner: NVIF_IOCTL_V0_OWNER_ANY,
            route: NVIF_IOCTL_V0_ROUTE_NVIF,
            token: 0,
            object: 0
        },
        new: NvifIoctlNewV0 {
            version: 0,
            pad01: Default::default(),
            route: NVIF_IOCTL_V0_ROUTE_NVIF,
            token: (&mut nouveau_object as *mut NouveauObject).addr() as u64,
            object: (&mut nouveau_object as *mut NouveauObject).addr() as u64,
            handle: 0,
            oclass: NV_DEVICE,
        },
        dev: NvDeviceV0 {
            version: 0,
            pad01: Default::default(),
            device: !0 // device identifier, ~0 for client default
        }
    };

    const DRM_NOUVEAU_NVIF: u32 = 0x07;
    drm_command_write(fd, DRM_NOUVEAU_NVIF, &init_args)?;

    #[repr(C)]
    struct NvifIoctlMthdV0 {
        version: u8,
        method: u8,
        pad02: [u8; 6],
    }
    #[repr(C)]
    #[derive(Debug)]
    struct NvifDeviceInfoV0 {
        version: u8,
        platform: u8,
        chipset: u16,
        revision: u8,
        family: u8,
        pad06: [u8; 2],
        ram_size: u64,
        ram_user: u64,
        chip: [u8; 16],
        name: [u8; 64],
    }
    #[repr(C)]
    struct DeviceInfoArgs {
        ioctl: NvifIoctlV0,
        mthd: NvifIoctlMthdV0,
        info: NvifDeviceInfoV0
    }
    const NVIF_IOCTL_V0_MTHD: u8 = 0x04;
    const NV_DEVICE_V0_INFO: u8 = 0x00;
    let mut device_info_args = DeviceInfoArgs {
        ioctl: NvifIoctlV0 {
            version: 0,
            r#type: NVIF_IOCTL_V0_MTHD,
            pad02: Default::default(),
            owner: NVIF_IOCTL_V0_OWNER_ANY,
            route: NVIF_IOCTL_V0_ROUTE_NVIF,
            token: 0,
            object: (&mut nouveau_object as *mut NouveauObject).addr() as u64
        },
        mthd: NvifIoctlMthdV0 {
            version: 0,
            method: NV_DEVICE_V0_INFO,
            pad02: Default::default(),
        },
        info: NvifDeviceInfoV0 {
            version: 0,
            platform: 0,
            chipset: 0,
            revision: 0,
            family: 0,
            pad06: Default::default(),
            ram_size: 0,
            ram_user: 0,
            chip: [0; 16],
            name: [0; 64],
        },
    };

    drm_command_read_write(fd, DRM_NOUVEAU_NVIF, &mut device_info_args)?;

    const NV_DEVICE_INFO_V0_IGP: u8 = 0x00;
    const NV_DEVICE_INFO_V0_SOC: u8 = 0x04;

    Ok(device_info_args.info.platform != NV_DEVICE_INFO_V0_IGP && device_info_args.info.platform != NV_DEVICE_INFO_V0_SOC)
}

fn is_discrete_gpu_xe(device: &GpuDevice) -> std::io::Result<bool> {
    let Some(node) = device.render.devnode() else {
        return Ok(false);
    };
    let file = std::fs::OpenOptions::new().read(true).write(true).open(node)?;
    use std::os::fd::AsRawFd;
    let fd = file.as_raw_fd();

    #[repr(C)]
    struct DrmXeDeviceQuery {
        extensions: u64,
        query: u32,
        size: u32,
        data: u64,
        _reserved: [u64; 2]
    }

    const DRM_XE_DEVICE_QUERY_CONFIG: u32 = 0x2;
    let mut device_query = DrmXeDeviceQuery {
        extensions: 0,
        query: DRM_XE_DEVICE_QUERY_CONFIG,
        size: 0,
        data: 0,
        _reserved: Default::default(),
    };

    const DRM_XE_DEVICE_QUERY: u32 = 0x00;

    // If size is set to 0, the driver fills it with the required size for the requested type of data to query.
    drm_command_read_write(fd, DRM_XE_DEVICE_QUERY, &mut device_query)?;

    const DRM_XE_QUERY_CONFIG_INFO_OFFSET: u32 = 1;
    const DRM_XE_QUERY_CONFIG_FLAGS: u32 = 1;

    if device_query.size < (DRM_XE_QUERY_CONFIG_INFO_OFFSET + DRM_XE_QUERY_CONFIG_FLAGS + 1) * std::mem::size_of::<u64>() as u32 {
        log::warn!("is_discrete_gpu_xe: query size is {}, not enough to read flags", device_query.size);
        return Ok(false);
    }

    let mut data = vec![0_u64; ((device_query.size+7)/8) as usize];
    device_query.data = data.as_mut_ptr().addr() as u64;

    // If size is equal to the required size, the queried information is copied into data.
    drm_command_read_write(fd, DRM_XE_DEVICE_QUERY, &mut device_query)?;

    let flags = data[(DRM_XE_QUERY_CONFIG_INFO_OFFSET + DRM_XE_QUERY_CONFIG_FLAGS) as usize];

    const DRM_XE_QUERY_CONFIG_FLAG_HAS_VRAM: u64 = 1 << 0;
    Ok((flags & DRM_XE_QUERY_CONFIG_FLAG_HAS_VRAM) != 0)
}
