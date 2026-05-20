use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::encoder::{
    AudioSettingsBuilder, ContainerSettingsBuilder, VideoEncoder, VideoSettingsBuilder,
};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};

/// Handles the capture events.
struct Capture {
    /// The video encoder used to encode the frames.
    encoder: Option<VideoEncoder>,
    /// The timestamp of when the capture started, used to calculate the recording duration.
    start: Instant,
    /// A flag to signal the capture thread to stop.
    stop_flag: Arc<AtomicBool>,
    /// The number of frames captured since the last FPS calculation.
    frame_count_since_reset: u64,
    /// The timestamp of the last FPS calculation, used to measure the interval.
    last_reset: Instant,
}

impl GraphicsCaptureApiHandler for Capture {
    /// The type of flags used to pass settings to the `new` function.
    type Flags = (Arc<AtomicBool>, String);

    /// The error type that can be returned from the capture handlers.
    type Error = Box<dyn std::error::Error + Send + Sync>;

    /// Called by the library to create a new instance of the handler.
    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        println!("Capture started. Press Ctrl+C to stop.");

        let (stop_flag, path) = ctx.flags;

        let monitor = Monitor::primary()?;
        let width = monitor.width()?;
        let height = monitor.height()?;

        let video_settings = VideoSettingsBuilder::new(width, height);
        let encoder = VideoEncoder::new(
            video_settings,
            // Disable audio for this example.
            AudioSettingsBuilder::default().disabled(true),
            ContainerSettingsBuilder::default(),
            &path,
        )?;

        Ok(Self {
            encoder: Some(encoder),
            start: Instant::now(),
            stop_flag,
            frame_count_since_reset: 0,
            last_reset: Instant::now(),
        })
    }

    /// Called for each new frame that is captured.
    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        self.frame_count_since_reset += 1;

        // Calculate the time elapsed since the last FPS reset.
        let elapsed_since_reset = self.last_reset.elapsed();
        // Calculate and display the current FPS.
        let fps = self.frame_count_since_reset as f64 / elapsed_since_reset.as_secs_f64();

        // Print the recording duration and current FPS.
        print!("Recording for: {:.2}s | FPS: {:.2}", self.start.elapsed().as_secs_f64(), fps);
        io::stdout().flush()?;

        // Send the frame to the video encoder.
        self.encoder.as_mut().unwrap().send_frame(frame)?;

        // Check if the stop flag has been set (e.g., by Ctrl+C).
        if self.stop_flag.load(Ordering::SeqCst) {
            println!(
                "
Stopping capture..."
            );

            // Finalize the encoding and save the video file.
            self.encoder.take().unwrap().finish()?;

            // Signal the capture loop to stop.
            capture_control.stop();

            println!(
                "
Recording stopped."
            );
        }

        // Reset the FPS counter every second.
        if elapsed_since_reset >= Duration::from_secs(1) {
            self.frame_count_since_reset = 0;
            self.last_reset = Instant::now();
        }

        Ok(())
    }

    /// Optional handler for when the capture item (e.g., a window) is closed.
    fn on_closed(&mut self) -> Result<(), Self::Error> {
        println!(
            "
Capture item closed, stopping capture."
        );

        // Stop the capture gracefully.
        self.stop_flag.store(true, Ordering::SeqCst);

        Ok(())
    }
}

fn main() {
    // Gets the primary monitor.
    let primary_monitor = Monitor::primary().expect("There is no primary monitor");
    let monitor_name = primary_monitor.name().expect("Failed to get monitor name");

    // Create an atomic boolean flag to signal the capture to stop.
    let stop_flag = Arc::new(AtomicBool::new(false));

    // Set up a Ctrl+C handler to gracefully stop the capture.
    {
        let stop_flag = stop_flag.clone();
        ctrlc::set_handler(move || {
            stop_flag.store(true, Ordering::SeqCst);
        })
        .expect("Failed to set Ctrl-C handler");
    }

    let settings = Settings::new(
        // The item to capture.
        primary_monitor,
        // The cursor capture settings.
        CursorCaptureSettings::Default,
        // The draw border settings.
        DrawBorderSettings::Default,
        // The secondary window settings.
        SecondaryWindowSettings::Default,
        // The minimum update interval.
        MinimumUpdateIntervalSettings::Default,
        // The dirty region settings.
        DirtyRegionSettings::Default,
        // The desired color format for the captured frame.
        ColorFormat::Bgra8,
        // The flags to pass to the `new` function of the handler.
        (stop_flag, format!("{monitor_name}.mp4")),
    );

    // Starts the capture and takes control of the current thread.
    // The errors from the handler trait will end up here.
    Capture::start(settings).expect("Screen capture failed");
}
