use iced::advanced::layout;
use iced::advanced::renderer;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget};
use iced::event::{self, Event};
use iced::mouse::Cursor;
use iced::overlay;
use iced::Size;
use iced::Vector;
use iced::{Element, Length, Rectangle};

const WIDTH: Length = Length::Shrink;
const HEIGHT: Length = Length::Shrink;

#[repr(transparent)]
struct State {
    hovered: bool,
}

pub(crate) struct Hoverable<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    on_hover: Option<Message>,
}

impl<'a, Message, Theme, Renderer> Hoverable<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    pub(crate) fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            content: content.into(),
            on_hover: None,
        }
    }

    pub(crate) fn on_hover(mut self, message: Message) -> Self {
        self.on_hover = Some(message);
        self
    }
}

impl<'a, Message: 'a, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Hoverable<'a, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: iced::advanced::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(WIDTH, HEIGHT)
    }

    #[inline]
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    #[inline]
    fn state(&self) -> tree::State {
        tree::State::new(State { hovered: false })
    }

    #[inline]
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    #[inline]
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        layout: Layout<'_>,
        cursor_position: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) -> event::Status {
        if let event::Status::Captured = self.content.as_widget_mut().on_event(
            &mut tree.children[0],
            event,
            layout.children().next().unwrap(),
            cursor_position,
            renderer,
            clipboard,
            shell,
            viewport,
        ) {
            return event::Status::Captured;
        }

        let state = tree.state.downcast_mut::<State>();

        match (
            state.hovered,
            cursor_position.position_over(layout.bounds()).is_some(),
        ) {
            (true, false) => {
                state.hovered = false;
            }
            (false, true) => {
                state.hovered = true;

                if let Some(on_hover) = &self.on_hover {
                    shell.publish(on_hover.clone());
                }
            }
            _ => {}
        }

        event::Status::Ignored
    }

    fn layout(
        &self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let limits = limits.width(WIDTH).height(HEIGHT);
        let content_layout =
            self.content
                .as_widget()
                .layout(&mut tree.children[0], renderer, &limits);
        let size = limits.resolve(WIDTH, HEIGHT, content_layout.size());
        layout::Node::with_children(size, vec![content_layout])
    }

    fn draw(
        &self,
        state: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor_position: Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let content_layout = layout.children().next().unwrap();

        self.content.as_widget().draw(
            &state.children[0],
            renderer,
            theme,
            style,
            content_layout,
            cursor_position,
            &bounds,
        );
    }

    fn mouse_interaction(
        &self,
        state: &Tree,
        layout: Layout<'_>,
        cursor_position: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> iced::advanced::mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &state.children[0],
            layout.children().next().unwrap(),
            cursor_position,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            translation,
        )
    }
}

impl<'a, Message: 'a, Theme: 'a, Renderer: 'a> From<Hoverable<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: iced::advanced::Renderer,
{
    #[inline]
    fn from(hoverable: Hoverable<'a, Message, Theme, Renderer>) -> Self {
        Self::new(hoverable)
    }
}
