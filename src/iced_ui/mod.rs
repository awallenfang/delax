use std::sync::Arc;

use nih_plug::editor::Editor;
use nih_plug::prelude::{AtomicF32, GuiContext};
use iced_baseview::*;
use crate::iced_ui::widgets::param_knob::{self, ParamKnob};
use crate::ui::InputData;
use crate::{Delax, DelaxParams};
mod widgets;

fn create_iced_editor<E: IcedEditor>(
    iced_state: Arc<IcedState>,
    initialization_flags: E::InitializationFlags,
) -> Option<Box<dyn Editor>> {
    let (parameter_updates_sender, parameter_updates_receiver) = channel::bounded(1);
}

pub(crate) fn default_state() -> Arc<IcedState> {
    IcedState::from_size(200, 150)
}

pub(crate) fn create(
    params: Arc<DelaxParams>,
    input_data: Arc<InputData>,
    editor_state: Arc<IcedState>,
) -> Option<Box<dyn Editor>> {
    create_iced_editor::<DelaxEditor>(editor_state, (params))
}

struct DelaxEditor {
    params: Arc<DelaxParams>,
    context: Arc<dyn GuiContext>,

    wetness_state: param_knob::State,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    /// Update a parameter's value.
    ParamUpdate(nih_widgets::ParamMessage),
}

impl IcedEditor for DelaxEditor {
    type Executor = executor::Default;
    type Message = Message;
    type InitializationFlags = (Arc<DelaxParams>);

    fn new(
        (params): Self::InitializationFlags,
        context: Arc<dyn GuiContext>,
    ) -> (Self, Command<Self::Message>) {
        let editor = DelaxEditor {
            params,
            context,

            wetness_state: Default::default(),
        };

        (editor, Command::none())
    }

    fn context(&self) -> &dyn GuiContext {
        self.context.as_ref()
    }

    fn update(
        &mut self,
        _window: &mut WindowQueue,
        message: Self::Message,
    ) -> Command<Self::Message> {
        match message {
            Message::ParamUpdate(message) => self.handle_param_message(message),
        }

        Command::none()
    }

    fn view(&mut self) -> Element<'_, Self::Message> {
        Column::new()
            .push(
                ParamKnob::new(&mut self.wetness_state, &self.params.wetness)
                    .map(Message::ParamUpdate),
            )
            .into()
    }
}
