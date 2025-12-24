use std::borrow::Borrow;

use nih_plug::params::Param;
use nih_plug_iced::backend::Renderer;
use nih_plug_iced::renderer::Renderer as GraphicsRenderer;
use nih_plug_iced::text::Renderer as TextRenderer;
use nih_plug_iced::widgets::ParamMessage;
use nih_plug_iced::*;

use atomic_refcell::AtomicRefCell;

use crate::iced_ui::widgets::util;

const BORDER_WIDTH: f32 = 1.0;

pub struct ParamKnob<'a, P: Param> {
    state: &'a mut State,

    param: &'a P,

    size: Length,
    text_size: Option<u16>,
    font: Font,
}

#[derive(Debug, Default)]
pub struct State {
    keyboard_modifiers: keyboard::Modifiers,
    /// Will be set to `true` if we're dragging the parameter. Resetting the parameter or entering a
    /// text value should not initiate a drag.
    drag_active: bool,
    /// We keep track of the start coordinate and normalized value holding down Shift while dragging
    /// for higher precision dragging. This is a `None` value when granular dragging is not active.
    granular_drag_start_y_value: Option<(f32, f32)>,
    /// Track clicks for double clicks.
    last_click: Option<mouse::Click>,
    last_pos: Option<Point>,

    /// State for the text input overlay that will be shown when this widget is alt+clicked.
    text_input_state: AtomicRefCell<widget::text_input::State>,
    /// The text that's currently in the text input. If this is set to `None`, then the text input
    /// is not visible.
    text_input_value: Option<String>,
}

#[derive(Debug, Clone)]
enum TextInputMessage {
    /// A new value was entered in the text input dialog.
    Value(String),
    /// Enter was pressed.
    Submit,
}

struct TextInputStyle;

impl widget::text_input::StyleSheet for TextInputStyle {
    fn active(&self) -> widget::text_input::Style {
        widget::text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border_radius: 0.0,
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        }
    }

    fn focused(&self) -> widget::text_input::Style {
        self.active()
    }

    fn placeholder_color(&self) -> Color {
        Color::from_rgb(0.7, 0.7, 0.7)
    }

    fn value_color(&self) -> Color {
        Color::from_rgb(0.3, 0.3, 0.3)
    }

    fn selection_color(&self) -> Color {
        Color::from_rgb(0.8, 0.8, 1.0)
    }
}

impl<'a, P: Param> ParamKnob<'a, P> {
    pub fn new(state: &'a mut State, param: &'a P) -> Self {
        Self {
            state,
            param,
            size: Length::Units(30),
            text_size: None,
            font: <Renderer as TextRenderer>::Font::default(),
        }
    }

    pub fn size(mut self, size: Length) -> Self {
        self.size = size;
        self
    }

    pub fn font(mut self, font: Font) -> Self {
        self.font = font;
        self
    }

    fn with_text_input<T, R, F>(&self, layout: Layout, renderer: R, current_value: &str, f: F) -> T
    where
        F: FnOnce(TextInput<'_, TextInputMessage>, Layout, R) -> T,
        R: Borrow<Renderer>,
    {
        let mut text_input_state = self.state.text_input_state.borrow_mut();
        text_input_state.focus();

        let text_size = self
            .text_size
            .unwrap_or_else(|| renderer.borrow().default_size());
        let text_width = renderer
            .borrow()
            .measure_width(current_value, text_size, self.font);
        let text_input = TextInput::new(
            &mut text_input_state,
            "",
            current_value,
            TextInputMessage::Value,
        )
        .font(self.font)
        .size(text_size)
        .width(Length::Units(text_width.ceil() as u16))
        .style(TextInputStyle)
        .on_submit(TextInputMessage::Submit);

        // Make sure to not draw over the borders, and center the text
        let offset_node = layout::Node::with_children(
            Size {
                width: text_width,
                height: layout.bounds().size().height - (BORDER_WIDTH * 2.0),
            },
            vec![layout::Node::new(layout.bounds().size())],
        );
        let offset_layout = Layout::with_offset(
            Vector {
                x: layout.bounds().center_x() - (text_width / 2.0),
                y: layout.position().y + BORDER_WIDTH,
            },
            &offset_node,
        );

        f(text_input, offset_layout, renderer)
    }

    fn set_normalized_value(&self, shell: &mut Shell<'_, ParamMessage>, normalized_value: f32) {
        // This snaps to the nearest plain value if the parameter is stepped in some way.
        // TODO: As an optimization, we could add a `const CONTINUOUS: bool` to the parameter to
        //       avoid this normalized->plain->normalized conversion for parameters that don't need
        //       it
        let plain_value = self.param.preview_plain(normalized_value);
        let current_plain_value = self.param.modulated_plain_value();
        if plain_value != current_plain_value {
            // For the aforementioned snapping
            let normalized_plain_value = self.param.preview_normalized(plain_value);
            shell.publish(ParamMessage::SetParameterNormalized(
                self.param.as_ptr(),
                normalized_plain_value,
            ));
        } else {
            shell.publish(ParamMessage::SetParameterNormalized(self.param.as_ptr(), normalized_value));
        }
    }
}

impl<'a, P: Param> Widget<ParamMessage, Renderer> for ParamKnob<'a, P> {
    fn width(&self) -> Length {
        self.size
    }

    fn height(&self) -> Length {
        self.size
    }

    fn layout(&self, _renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        let limits = limits.width(self.size).height(self.size);
        let size = limits.resolve(Size::ZERO);

        layout::Node::new(size)
    }

    fn on_event(
        &mut self,
        event: Event,
        layout: Layout<'_>,
        cursor_position: Point,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, ParamMessage>,
    ) -> event::Status {

        let text_input_status = if let Some(current_value) = &self.state.text_input_value {
            let event = event.clone();
            let mut messages = Vec::new();
            let mut text_input_shell = Shell::new(&mut messages);
            let status = self.with_text_input(
                layout,
                renderer,
                current_value,
                |mut text_input, layout, renderer| {
                    text_input.on_event(
                        event,
                        layout,
                        cursor_position,
                        renderer,
                        clipboard,
                        &mut text_input_shell,
                    )
                },
            );

            // Pressing escape will unfocus the text field, so we should propagate that change in
            // our own model
            if self.state.text_input_state.borrow().is_focused() {
                for message in messages {
                    match message {
                        TextInputMessage::Value(s) => self.state.text_input_value = Some(s),
                        TextInputMessage::Submit => {
                            if let Some(normalized_value) = self
                                .state
                                .text_input_value
                                .as_ref()
                                .and_then(|s| self.param.string_to_normalized_value(s))
                            {
                                shell.publish(ParamMessage::BeginSetParameter(self.param.as_ptr()));
                                self.set_normalized_value(shell, normalized_value);
                                shell.publish(ParamMessage::EndSetParameter(self.param.as_ptr()));
                            }

                            // And defocus the text input widget again
                            self.state.text_input_value = None;
                        }
                    }
                }
            } else {
                self.state.text_input_value = None;
            }

            status
        } else {
            event::Status::Ignored
        };

        if text_input_status == event::Status::Captured {
            return event::Status::Captured;
        }

        // Compensate for the border when handling these events
        let bounds = layout.bounds();
        let bounds = Rectangle {
            x: bounds.x + BORDER_WIDTH,
            y: bounds.y + BORDER_WIDTH,
            width: bounds.width - (BORDER_WIDTH * 2.0),
            height: bounds.height - (BORDER_WIDTH * 2.0),
        };

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if bounds.contains(cursor_position) {
                    self.state.last_pos = Some(cursor_position);
                    println!("Setting last_pos");
                    let click = mouse::Click::new(cursor_position, self.state.last_click);
                    self.state.last_click = Some(click);

                    if self.state.keyboard_modifiers.alt() {
                        // Alt+click should not start a drag, instead it should show the text entry
                        // widget
                        self.state.drag_active = false;

                        // Changing the parameter happens in the TextInput event handler above
                        let mut text_input_state = self.state.text_input_state.borrow_mut();
                        self.state.text_input_value = Some(self.param.to_string());
                        text_input_state.move_cursor_to_end();
                        text_input_state.select_all();
                    } else if self.state.keyboard_modifiers.command()
                        || matches!(click.kind(), mouse::click::Kind::Double)
                    {
                        // Likewise resetting a parameter should not let you immediately drag it to a new value
                        self.state.drag_active = false;

                        shell.publish(ParamMessage::BeginSetParameter(self.param.as_ptr()));
                        self.set_normalized_value(shell, self.param.default_normalized_value());
                        shell.publish(ParamMessage::EndSetParameter(self.param.as_ptr()));
                    } else {
                        self.state.drag_active = true;

                    }

                    return event::Status::Captured;
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. }) => {
                if self.state.drag_active {
                    self.state.drag_active = false;

                    return event::Status::Captured;
                }
                self.state.last_pos = None;
            }
            Event::Mouse(mouse::Event::CursorMoved { .. })
            | Event::Touch(touch::Event::FingerMoved { .. }) => {
                // Don't do anything when we just reset the parameter because that would be weird
                if self.state.drag_active {
                    if let Some(last_pos) = self.state.last_pos {
                        let param_val = self.param.modulated_normalized_value();
                        let delta: f32 = cursor_position.y - last_pos.y;
                        
                        shell.publish(ParamMessage::BeginSetParameter(self.param.as_ptr()));
                        shell.publish(ParamMessage::SetParameterNormalized(self.param.as_ptr(), param_val + delta * 0.01));
                        shell.publish(ParamMessage::EndSetParameter(self.param.as_ptr()));
                        self.state.last_pos = Some(cursor_position);
                    }

                    self.state.last_click =
                        Some(mouse::Click::new(cursor_position, self.state.last_click));
                    self.state.last_pos = Some(cursor_position);
                    return event::Status::Captured;
                }
            }
            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                self.state.keyboard_modifiers = modifiers;

                return event::Status::Captured;
            }
            _ => {}
        }

        event::Status::Ignored
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor_position: Point,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let is_mouse_over = bounds.contains(cursor_position);
        renderer.draw_primitive(primitive);
        renderer.fill_quad(renderer::Quad{
            bounds,
            border_width: BORDER_WIDTH,
            border_radius: bounds.width / 2.,
            border_color: Color::BLACK
        }, Color::WHITE);
        // I'm sure there's some philosophical meaning behind this
        let bounds_without_borders = Rectangle {
            x: bounds.x + BORDER_WIDTH,
            y: bounds.y + BORDER_WIDTH,
            width: bounds.width - (BORDER_WIDTH * 2.0),
            height: bounds.height - (BORDER_WIDTH * 2.0),
        };

        // The bar itself, show a different background color when the value is being edited or when
        // the mouse is hovering over it to indicate that it's interactive
        let background_color =
            if is_mouse_over || self.state.drag_active || self.state.text_input_value.is_some() {
                Color::new(0.5, 0.5, 0.5, 0.1)
            } else {
                Color::TRANSPARENT
            };

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border_color: Color::BLACK,
                border_width: BORDER_WIDTH,
                border_radius: 0.0,
            },
            background_color,
        );

        // Only draw the text input widget when it gets focussed. Otherwise, overlay the label with
        // the slider.
        if let Some(current_value) = &self.state.text_input_value {
            self.with_text_input(
                layout,
                renderer,
                current_value,
                |text_input, layout, renderer| {
                    text_input.draw(renderer, layout, cursor_position, None)
                },
            )
        } else {
            // We'll visualize the difference between the current value and the default value if the
            // default value lies somewhere in the middle and the parameter is continuous. Otherwise
            // this appraoch looks a bit jarring.
            let current_value = self.param.modulated_normalized_value();
            let default_value = self.param.default_normalized_value();
            let fill_start_x = util::remap_rect_x_t(
                &bounds_without_borders,
                if self.param.step_count().is_none() && (0.45..=0.55).contains(&default_value) {
                    default_value
                } else {
                    0.0
                },
            );
            let fill_end_x = util::remap_rect_x_t(&bounds_without_borders, current_value);

            let fill_color = Color::from_rgb8(196, 196, 196);
            let fill_rect = Rectangle {
                x: fill_start_x.min(fill_end_x),
                width: (fill_end_x - fill_start_x).abs(),
                ..bounds_without_borders
            };
            renderer.fill_quad(
                renderer::Quad {
                    bounds: fill_rect,
                    border_color: Color::TRANSPARENT,
                    border_width: 0.0,
                    border_radius: 0.0,
                },
                fill_color,
            );

            // To make it more readable (and because it looks cool), the parts that overlap with the
            // fill rect will be rendered in white while the rest will be rendered in black.
            let display_value = self.param.to_string();
            let text_size = self.text_size.unwrap_or_else(|| renderer.default_size()) as f32;
            let text_bounds = Rectangle {
                x: bounds.center_x(),
                y: bounds.center_y(),
                ..bounds
            };
            renderer.fill_text(text::Text {
                content: &display_value,
                font: self.font,
                size: text_size,
                bounds: text_bounds,
                color: style.text_color,
                horizontal_alignment: alignment::Horizontal::Center,
                vertical_alignment: alignment::Vertical::Center,
            });

            // This will clip to the filled area
            renderer.with_layer(fill_rect, |renderer| {
                let filled_text_color = Color::from_rgb8(80, 80, 80);
                renderer.fill_text(text::Text {
                    content: &display_value,
                    font: self.font,
                    size: text_size,
                    bounds: text_bounds,
                    color: filled_text_color,
                    horizontal_alignment: alignment::Horizontal::Center,
                    vertical_alignment: alignment::Vertical::Center,
                });
            });
        }
    }
}


impl<'a, P: Param> ParamKnob<'a, P> {
    /// Convert this [`ParamKnob`] into an [`Element`] with the correct message. You should have a
    /// variant on your own message type that wraps around [`ParamMessage`] so you can forward those
    /// messages to
    /// [`IcedEditor::handle_param_message()`][crate::IcedEditor::handle_param_message()].
    pub fn map<Message, F>(self, f: F) -> Element<'a, Message>
    where
        Message: 'static,
        F: Fn(ParamMessage) -> Message + 'static,
    {
        Element::from(self).map(f)
    }
}

impl<'a, P: Param> From<ParamKnob<'a, P>> for Element<'a, ParamMessage> {
    fn from(widget: ParamKnob<'a, P>) -> Self {
        Element::new(widget)
    }
}
