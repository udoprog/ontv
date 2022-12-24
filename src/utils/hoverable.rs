use iced::overlay;
use iced_native::event::{self, Event};
use iced_native::layout;
use iced_native::renderer;
use iced_native::widget::tree::{self, Tree};
use iced_native::{Clipboard, Element, Layout, Length, Point, Rectangle, Shell, Widget};

const WIDTH: Length = Length::Shrink;
const HEIGHT: Length = Length::Shrink;

#[repr(transparent)]
struct State {
    hovered: bool,
}

pub(crate) struct Hoverable<'a, Message, Renderer> {
    content: Element<'a, Message, Renderer>,
    on_hover: Option<Message>,
}

impl<'a, Message, Renderer> Hoverable<'a, Message, Renderer>
where
    Renderer: iced_native::Renderer,
{
    pub(crate) fn new(content: impl Into<Element<'a, Message, Renderer>>) -> Self {
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

impl<'a, Message: 'a, Renderer> Widget<Message, Renderer> for Hoverable<'a, Message, Renderer>
where
    Message: Clone,
    Renderer: iced_native::Renderer,
{
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
        cursor_position: Point,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) -> event::Status {
        if let event::Status::Captured = self.content.as_widget_mut().on_event(
            &mut tree.children[0],
            event,
            layout.children().next().unwrap(),
            cursor_position,
            renderer,
            clipboard,
            shell,
        ) {
            return event::Status::Captured;
        }

        let mut state = tree.state.downcast_mut::<State>();

        match (state.hovered, layout.bounds().contains(cursor_position)) {
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

    fn layout(&self, renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        let limits = limits.width(WIDTH).height(HEIGHT);
        let content_layout = self.content.as_widget().layout(renderer, &limits);
        let size = limits.resolve(content_layout.size());
        layout::Node::with_children(size, vec![content_layout])
    }

    #[inline]
    fn width(&self) -> Length {
        WIDTH
    }

    #[inline]
    fn height(&self) -> Length {
        HEIGHT
    }

    fn draw(
        &self,
        state: &Tree,
        renderer: &mut Renderer,
        theme: &<Renderer as iced_native::Renderer>::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor_position: Point,
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
        cursor_position: Point,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> iced_native::mouse::Interaction {
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
    ) -> Option<overlay::Element<'b, Message, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
        )
    }
}

impl<'a, Message: 'a, Renderer: 'a> From<Hoverable<'a, Message, Renderer>>
    for Element<'a, Message, Renderer>
where
    Message: Clone,
    Renderer: iced_native::Renderer,
{
    #[inline]
    fn from(hoverable: Hoverable<'a, Message, Renderer>) -> Self {
        Self::new(hoverable)
    }
}
