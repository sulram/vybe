//! THE SHELL — the winit loop: opens the window, pumps frames, and turns the
//! window's events into the core's own [`Event`]s. Hidden from the sketch,
//! like everything that isn't a knob.

use std::path::PathBuf;
use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use crate::gpu::State;
use crate::input::{Binding, Event, Inputs, Io, Key, Value};
use crate::recipe::Recipe;
use crate::stage::{Clock, Render, Warp};

/// Whatever describes the picture, frame after frame.
pub(crate) enum Source {
    /// A chain, described once.
    Still(Recipe),
    /// A live sketch: the chain as a function, re-described whenever a tuned
    /// knob changes.
    Live(Box<dyn Fn() -> Recipe>),
    /// A performance: re-described *every* frame, from the inputs and the
    /// clock (the patch player).
    Play(Box<dyn Perform>),
}

/// Something that turns this frame's inputs and time into a recipe, holding
/// whatever state that takes (ramps, gates, which scene is showing).
pub(crate) trait Perform {
    fn frame(&mut self, inputs: &Inputs, time: f32, dt: f32) -> Recipe;
    /// The image files it will show — loaded before the first frame.
    fn media(&self) -> Vec<PathBuf>;
}

/// Everything a run needs: what to show, who is listening, and the window's
/// wishes. Built by the terminal links and by `Stage`.
pub(crate) struct Show {
    pub source: Source,
    pub bindings: Vec<Box<dyn Binding>>,
    /// Logical window size.
    pub size: [f32; 2],
    pub title: Option<String>,
}

impl Show {
    pub(crate) fn new(source: Source) -> Self {
        Self {
            source,
            bindings: Vec::new(),
            size: [800.0, 800.0],
            title: None,
        }
    }

    /// The recipe for this frame, or `None` when the picture's description
    /// hasn't changed (the GPU keeps animating it on its own).
    pub(crate) fn describe(
        &mut self,
        inputs: &Inputs,
        time: f32,
        dt: f32,
        first: bool,
    ) -> Option<Recipe> {
        match &mut self.source {
            Source::Still(recipe) => first.then(|| recipe.clone()),
            // A knob turned since last frame? Re-describe the sketch. (Taken
            // on the first frame too: describing registers the knobs.)
            Source::Live(sketch) => (crate::tune::take_dirty() || first).then(sketch),
            Source::Play(player) => Some(player.frame(inputs, time, dt)),
        }
    }

    pub(crate) fn media(&self) -> Vec<PathBuf> {
        match &self.source {
            Source::Play(player) => player.media(),
            _ => Vec::new(),
        }
    }

    /// Runs the show: in a window — or, when `VYBE_RENDER` is set, headless to
    /// PNGs, so every sketch can be rendered without a line of its own.
    pub(crate) fn run(self) {
        match Render::from_env() {
            Some(render) => match render.run(self) {
                Ok(frames) => frames.iter().for_each(|f| println!("{}", f.display())),
                Err(e) => eprintln!("vybe: {e}"),
            },
            None => self.open(),
        }
    }

    fn open(self) {
        let event_loop = EventLoop::new().unwrap();
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
        let mut app = App {
            show: self,
            state: None,
            clock: Clock::wall(),
            inputs: Inputs::default(),
            warp: Warp::default(),
            pointer: [1e9, 1e9],
            shift: false,
            last: (0.0, 0.0),
        };
        event_loop.run_app(&mut app).unwrap();
    }
}

/// The winit 0.30 application "shell": creates the window in `resumed`,
/// brings up the GPU, and routes window events to the [`State`].
struct App {
    show: Show,
    state: Option<State>,
    clock: Clock,
    inputs: Inputs,
    warp: Warp,
    /// Where the pointer last was, scene space.
    pointer: [f32; 2],
    shift: bool,
    /// The last frame's (time, dt) — what an event between frames is stamped with.
    last: (f32, f32),
}

impl App {
    /// Hands an event to every binding, and mirrors it into the inputs.
    fn dispatch(&mut self, event: Event) {
        match event {
            Event::Key {
                key,
                pressed,
                repeat,
                ..
            } => {
                let held = if pressed { 1.0 } else { 0.0 };
                self.inputs
                    .set(&format!("/key/{}", key.name()), Value::Num(held));
                if pressed && !repeat {
                    crate::tune::cycle(key);
                }
            }
            Event::Pointer { at } => {
                self.inputs.set("/mouse/x", Value::Num(at[0]));
                self.inputs.set("/mouse/y", Value::Num(at[1]));
            }
            Event::Button { pressed, .. } => {
                let held = if pressed { 1.0 } else { 0.0 };
                self.inputs.set("/mouse/down", Value::Num(held));
            }
        }
        let mut io = Io {
            inputs: &mut self.inputs,
            warp: &mut self.warp,
            time: self.last.0,
            dt: self.last.1,
            title: None,
        };
        for binding in &mut self.show.bindings {
            binding.event(&event, &mut io);
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let recipe = self
            .show
            .describe(&self.inputs, 0.0, 0.0, true)
            .unwrap_or(Recipe::Shapes(Vec::new()));
        let title = self.show.title.clone();
        let attrs = Window::default_attributes()
            .with_title(title.as_deref().unwrap_or(recipe.title()))
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.show.size[0],
                self.show.size[1],
            ));
        let window = Arc::new(event_loop.create_window(attrs).unwrap());
        #[allow(unused_mut)]
        let mut state = pollster::block_on(State::new(window.clone(), recipe));
        state.engine().preload(&self.show.media());

        // A live sketch with picked knobs gets the tweak panel — an Overlay
        // like any other; the core below this point knows nothing of egui.
        #[cfg(feature = "tweak")]
        if matches!(self.show.source, Source::Live(_)) && crate::tune::any() {
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
            WindowEvent::ModifiersChanged(mods) => {
                self.shift = mods.state().contains(ModifiersState::SHIFT);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = state.pointer(position, consumed);
                if !consumed {
                    self.dispatch(Event::Pointer { at: self.pointer });
                }
            }
            // Cursor left the window → the mouse signal returns to rest, so
            // at(mouse())/grow(mouse()) don't freeze at the last edge position.
            WindowEvent::CursorLeft { .. } => state.rest_mouse(),
            WindowEvent::MouseInput {
                state: button_state,
                button: MouseButton::Left,
                ..
            } if !consumed => self.dispatch(Event::Button {
                pressed: button_state == ElementState::Pressed,
                at: self.pointer,
            }),
            WindowEvent::KeyboardInput { event: key, .. } if !consumed => {
                if let Some(named) = translate(&key.logical_key) {
                    self.dispatch(Event::Key {
                        key: named,
                        pressed: key.state == ElementState::Pressed,
                        repeat: key.repeat,
                        shift: self.shift,
                    });
                }
            }
            WindowEvent::RedrawRequested => {
                let (time, dt) = self.clock.tick();
                self.last = (time, dt);
                let mut io = Io {
                    inputs: &mut self.inputs,
                    warp: &mut self.warp,
                    time,
                    dt,
                    title: None,
                };
                for binding in &mut self.show.bindings {
                    binding.frame(&mut io);
                }
                if let Some(title) = io.title.take() {
                    state.window.set_title(&title);
                }
                if let Some(recipe) = self.show.describe(&self.inputs, time, dt, false) {
                    state.engine().set_recipe(recipe);
                }
                state.set_warp(self.warp);
                state.render(time, dt);
                state.window.request_redraw(); // request the next frame → animates
            }
            _ => {}
        }
    }
}

/// winit's key -> ours. Keys we have no name for are simply not events.
fn translate(key: &WinitKey) -> Option<Key> {
    Some(match key {
        WinitKey::Named(NamedKey::ArrowLeft) => Key::Left,
        WinitKey::Named(NamedKey::ArrowRight) => Key::Right,
        WinitKey::Named(NamedKey::ArrowUp) => Key::Up,
        WinitKey::Named(NamedKey::ArrowDown) => Key::Down,
        WinitKey::Named(NamedKey::Tab) => Key::Tab,
        WinitKey::Named(NamedKey::Space) => Key::Space,
        WinitKey::Named(NamedKey::Enter) => Key::Enter,
        WinitKey::Named(NamedKey::Escape) => Key::Escape,
        WinitKey::Character(text) => {
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(' '), None) => Key::Space,
                (Some(c), None) => Key::Char(c.to_ascii_lowercase()),
                _ => return None,
            }
        }
        _ => return None,
    })
}
