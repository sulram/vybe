//! THE SHELL — the winit loop: opens the window, pumps frames, and feeds
//! input to the GPU core. Hidden from the sketch, like everything that isn't
//! a knob.

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use crate::gpu::State;
use crate::recipe::Recipe;

/// A live sketch: the chain as a function, re-described whenever a tuned
/// knob changes.
type Sketch = Box<dyn Fn() -> Recipe>;

/// The winit 0.30 application "shell": creates the window in `resumed`,
/// brings up the GPU, and routes window events to the [`State`].
struct App {
    recipe: Recipe,
    sketch: Option<Sketch>,
    state: Option<State>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title(self.recipe.title())
            .with_inner_size(winit::dpi::LogicalSize::new(800.0, 800.0));
        let window = Arc::new(event_loop.create_window(attrs).unwrap());
        #[allow(unused_mut)]
        let mut state = pollster::block_on(State::new(window.clone(), self.recipe.clone()));

        // A live sketch with picked knobs gets the tweak panel — an Overlay
        // like any other; the core below this point knows nothing of egui.
        #[cfg(feature = "tweak")]
        if self.sketch.is_some() && crate::tune::any() {
            state.set_overlay(Box::new(crate::tweak::Panel::new(
                state.device(),
                state.surface_format(),
                &window,
            )));
        }

        window.request_redraw();
        self.state = Some(state);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        // The overlay sees events first; when it consumes one (pointer over
        // a slider), the scene's mouse doesn't move.
        let consumed = state.overlay_event(&event);

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size),
            WindowEvent::CursorMoved { position, .. } => {
                if !consumed {
                    state.set_mouse(position);
                }
            }
            // Cursor left the window → the mouse signal returns to rest, so
            // at(mouse())/grow(mouse()) don't freeze at the last edge position.
            WindowEvent::CursorLeft { .. } => state.rest_mouse(),
            WindowEvent::RedrawRequested => {
                // A knob turned since last frame? Re-describe the sketch.
                if let Some(sketch) = &self.sketch {
                    if crate::tune::take_dirty() {
                        state.set_recipe(sketch());
                    }
                }
                state.render();
                state.window.request_redraw(); // request the next frame → animates
            }
            _ => {}
        }
    }
}

/// Entry point called by the chains' terminal links.
pub(crate) fn run(recipe: Recipe) {
    run_app(recipe, None);
}

/// Entry point for live sketches (see `sugar::live`). Describing once up
/// front registers the tuned knobs and yields the initial recipe.
pub(crate) fn run_live(sketch: Sketch) {
    let recipe = sketch();
    run_app(recipe, Some(sketch));
}

fn run_app(recipe: Recipe, sketch: Option<Sketch>) {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let mut app = App {
        recipe,
        sketch,
        state: None,
    };
    event_loop.run_app(&mut app).unwrap();
}
