use iced::{overlay, Padding};
use iced_native::event::{self, Event};
use iced_native::layout;
use iced_native::renderer;
use iced_native::widget::tree::{self, Tree};
use iced_native::{Clipboard, Element, Layout, Length, Point, Rectangle, Shell, Widget};

#[allow(missing_debug_implementations)]
pub struct Hoverable<'a, Message, Renderer> {
    content: Element<'a, Message, Renderer>,
    on_hover: Message,
    on_unhover: Message,
    padding: Padding,
}

impl<'a, Message, Renderer> Hoverable<'a, Message, Renderer>
where
    Renderer: iced_native::Renderer,
{
    const WIDTH: Length = Length::Shrink;
    const HEIGHT: Length = Length::Shrink;

    pub fn new(
        content: Element<'a, Message, Renderer>,
        on_hover: Message,
        on_unhover: Message,
    ) -> Self {
        Self {
            content,
            on_hover,
            on_unhover,
            padding: Padding::ZERO,
        }
    }

    pub fn padding<P>(mut self, padding: P) -> Self
    where
        P: Into<Padding>,
    {
        self.padding = padding.into();
        self
    }
}

impl<'a, Message, Renderer> Widget<Message, Renderer> for Hoverable<'a, Message, Renderer>
where
    Message: 'a + Clone,
    Renderer: iced_native::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

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
        let was_hovered = state.is_hovered;
        let now_hovered = layout.bounds().contains(cursor_position);

        match (was_hovered, now_hovered) {
            (true, true) => {}
            (false, false) => {}
            (true, false) => {
                // exited hover
                state.is_hovered = now_hovered;
                shell.publish(self.on_unhover.clone());
            }
            (false, true) => {
                // entered hover
                state.is_hovered = now_hovered;
                shell.publish(self.on_hover.clone());
            }
        }

        event::Status::Ignored
    }

    fn layout(&self, renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        let limits = limits
            .width(Self::WIDTH)
            .height(Self::HEIGHT)
            .pad(self.padding);

        let mut content_layout = self.content.as_widget().layout(renderer, &limits);
        content_layout.move_to(Point::new(
            self.padding.left.into(),
            self.padding.top.into(),
        ));

        let size = limits.resolve(content_layout.size()).pad(self.padding);

        layout::Node::with_children(size, vec![content_layout])
    }

    fn width(&self) -> Length {
        Self::WIDTH
    }

    fn height(&self) -> Length {
        Self::HEIGHT
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct State {
    is_hovered: bool,
}

impl<'a, Message, Renderer> From<Hoverable<'a, Message, Renderer>>
    for Element<'a, Message, Renderer>
where
    Message: Clone + 'a,
    Renderer: iced_native::Renderer + 'a,
{
    fn from(hoverable: Hoverable<'a, Message, Renderer>) -> Self {
        Self::new(hoverable)
    }
}
