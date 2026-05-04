use as_raw_xcb_connection::AsRawXcbConnection;
use tiny_xlib::Display;
use xcb::{x, Xid};

xcb::atoms_struct! {
    #[derive(Debug)]
    struct Atoms {
        wm_protocols    => b"WM_PROTOCOLS",
        wm_del_window   => b"WM_DELETE_WINDOW",
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open a display.
    let display = Display::new(None)?;

    // Get the XCB connection.
    let conn = display.as_raw_xcb_connection();

    // Use that pointer to create a new XCB connection.
    let conn =
        unsafe { xcb::Connection::from_raw_conn_and_extensions_no_drop(conn.cast(), &[], &[]) };

    // Register a handler for X11 errors.
    let _ = tiny_xlib::register_error_handler(Box::new(|_, error| {
        println!("X11 error: {:?}", error);
        false
    }));

    // Now we have a Xlib and XCB setup using tiny-xlib. We could now initialize OpenGL for instance.
    // For this example, we will just display a simple window.

    // A window just for example purposes.
    let setup = conn.get_setup();
    let screen = setup.roots().nth(display.screen_index() as usize).unwrap();

    let window: x::Window = conn.generate_id();

    conn.send_request(&x::CreateWindow {
        depth: x::COPY_FROM_PARENT as u8,
        wid: window,
        parent: screen.root(),
        x: 0,
        y: 0,
        width: 150,
        height: 150,
        border_width: 10,
        class: x::WindowClass::InputOutput,
        visual: screen.root_visual(),
        value_list: &[
            x::Cw::BackPixel(screen.white_pixel()),
            x::Cw::EventMask(x::EventMask::EXPOSURE | x::EventMask::KEY_PRESS),
        ],
    });

    conn.send_request(&x::MapWindow { window });

    conn.flush()?;

    // retrieving a few atoms
    let atoms = Atoms::intern_all(&conn)?;
    println!("atoms = {:#?}", atoms);

    // activate the sending of close event through `x::Event::ClientMessage`
    // either the request must be checked as follow, or conn.flush() must be called before entering the loop
    conn.send_and_check_request(&x::ChangeProperty {
        mode: x::PropMode::Replace,
        window,
        property: atoms.wm_protocols,
        r#type: x::ATOM_ATOM,
        data: &[atoms.wm_del_window],
    })?;

    // Do whatever you want with the XCB connection.
    loop {
        let ev = conn.wait_for_event()?;
        println!("Event: {:?}", ev);

        match ev {
            xcb::Event::X(x::Event::ClientMessage(ev)) => {
                if let x::ClientMessageData::Data32([atom, ..]) = ev.data() {
                    if atom == atoms.wm_del_window.resource_id() {
                        // window "x" button clicked by user, we gracefully exit
                        break Ok(());
                    }
                }
            }
            _ => {}
        }
    }
}
