mod app;
pub mod color;
mod renderer;

use std::path::Path;
use std::sync::mpsc::{channel, Sender, TryRecvError};
use std::time::Duration;
use std::{
    borrow::BorrowMut,
    collections::HashMap,
    sync::{Arc, Mutex},
};

use color_eyre::eyre::Result;
use dashmap::DashMap;
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEvent, Debouncer};
use renderer::Renderer;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder, WindowId},
};

use app::App;

// Add this function to main.rs
fn watch_shader_files(
    sender: Sender<std::result::Result<Vec<DebouncedEvent>, Vec<notify::Error>>>,
) -> Result<Debouncer<notify::ReadDirectoryChangesWatcher>> {
    let mut debouncer = new_debouncer(Duration::from_millis(250), None, sender)?;

    debouncer
        .watcher()
        .watch(Path::new("src/shaders"), RecursiveMode::Recursive)?;

    Ok(debouncer)
}

#[pollster::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let event_loop = EventLoop::new();
    let egui_state = egui_winit::State::new(&event_loop);
    let mut egui_state = Arc::new(Mutex::new(egui_state));
    let window = WindowBuilder::new()
        .with_title("Shader Playground")
        .build(&event_loop)
        .unwrap();

    let mut viewport_map: HashMap<WindowId, (Window, wgpu::Color)> = HashMap::new();
    let window_id = window.id();
    viewport_map.insert(window.id(), (window, wgpu::Color::RED));

    let contexts: DashMap<WindowId, _> = viewport_map
        .keys()
        .map(|id| (*id, egui::Context::default()))
        .collect();
    let contexts = Arc::new(contexts);

    let mut app = App::new();
    let (win, col) = viewport_map.get(&window_id).unwrap();
    let mut renderer = Renderer::new(&mut [(win, *col)], Arc::clone(&contexts)).await?;

    let (sender, receiver) = channel();
    let _watcher = watch_shader_files(sender)?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::MainEventsCleared => {
                for (window, _) in viewport_map.values() {
                    window.request_redraw();
                }
            }
            Event::RedrawRequested(window_id) => {
                if let Some((window, _)) = viewport_map.get_mut(&window_id) {
                    renderer.render(&mut app, window, Arc::clone(&egui_state));
                }
            }
            Event::WindowEvent {
                window_id,
                event: WindowEvent::Resized(size),
                ..
            } => {
                if let Some((window, _)) = viewport_map.get_mut(&window_id) {
                    renderer.resize(window, size);
                }
            }
            Event::WindowEvent { window_id, event } => {
                if let Some(ctx) = contexts.get(&window_id) {
                    _ = egui_state
                        .borrow_mut()
                        .lock()
                        .unwrap()
                        .on_event(&ctx, &event);
                }
                app.handle_window_event(window_id, &event);
            }
            _ => (),
        }

        match receiver.try_recv() {
            Ok(_events) => {
                // Ok(DebouncedEvent::Write(_)) | Ok(DebouncedEvent::Create(_)) => {
                println!("Shader file changed. Reloading shaders...");
                let (_win, _col) = viewport_map.get(&window_id).unwrap();
                renderer.reload().unwrap();
                Ok(())
            }
            Err(err) => match err {
                TryRecvError::Empty => Ok(()),
                TryRecvError::Disconnected => Err(err),
            },
        }
        .unwrap();
    });
}
