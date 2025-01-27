//! # Ids
//!
//! [Id](id::Id)s are unique identifiers for views.
//! They're used to identify views in the view tree.
//!
//! ## Ids and Id paths
//!
//! These ids are assigned via the [ViewContext](context::ViewContext) and are unique across the entire application.
//!

use std::{any::Any, cell::RefCell, collections::HashMap, num::NonZeroU64};

use crate::{
    animate::Animation,
    app_handle::{StyleSelector, UpdateMessage, DEFERRED_UPDATE_MESSAGES, UPDATE_MESSAGES},
    context::{EventCallback, MenuCallback, ResizeCallback},
    event::EventListener,
    responsive::ScreenSize,
    style::Style,
};

thread_local! {
    pub(crate) static ID_PATHS: RefCell<HashMap<Id,IdPath>> = Default::default();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Hash)]
pub struct WindowId(NonZeroU64);

impl WindowId {
    /// Allocate a new, unique `Id`.
    pub fn next() -> WindowId {
        use glazier::Counter;
        static WIDGET_ID_COUNTER: Counter = Counter::new();
        WindowId(WIDGET_ID_COUNTER.next_nonzero())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Hash)]
/// A stable identifier for an element.
pub struct Id(NonZeroU64);

#[derive(Clone, Default)]
pub struct IdPath(pub(crate) Vec<Id>);

impl Id {
    /// Allocate a new, unique `Id`.
    pub fn next() -> Id {
        use glazier::Counter;
        static WIDGET_ID_COUNTER: Counter = Counter::new();
        Id(WIDGET_ID_COUNTER.next_nonzero())
    }

    #[allow(unused)]
    pub fn to_raw(self) -> u64 {
        self.0.into()
    }

    pub fn to_nonzero_raw(self) -> NonZeroU64 {
        self.0
    }

    pub fn new(&self) -> Id {
        let mut id_path =
            ID_PATHS.with(|id_paths| id_paths.borrow().get(self).cloned().unwrap_or_default());
        let new_id = if id_path.0.is_empty() {
            // if id_path is empty, it means the id was generated by next() and it's not
            // tracked yet, so we can just reuse it
            *self
        } else {
            Self::next()
        };
        id_path.0.push(new_id);
        ID_PATHS.with(|id_paths| {
            id_paths.borrow_mut().insert(new_id, id_path);
        });
        new_id
    }

    pub fn parent(&self) -> Option<Id> {
        ID_PATHS.with(|id_paths| {
            id_paths.borrow().get(self).and_then(|id_path| {
                let id_path = &id_path.0;
                let len = id_path.len();
                if len >= 2 {
                    Some(id_path[len - 2])
                } else {
                    None
                }
            })
        })
    }

    pub fn id_path(&self) -> Option<IdPath> {
        ID_PATHS.with(|id_paths| id_paths.borrow().get(self).cloned())
    }

    pub fn has_id_path(&self) -> bool {
        ID_PATHS.with(|id_paths| id_paths.borrow().contains_key(self))
    }

    pub fn remove_id_path(&self) {
        ID_PATHS.with(|id_paths| id_paths.borrow_mut().remove(self));
    }

    pub fn root_id(&self) -> Option<Id> {
        ID_PATHS.with(|id_paths| {
            id_paths
                .borrow()
                .get(self)
                .and_then(|path| path.0.first().copied())
        })
    }

    pub fn request_focus(&self) {
        self.add_update_message(UpdateMessage::Focus(*self));
    }

    pub fn request_active(&self) {
        self.add_update_message(UpdateMessage::Active(*self));
    }

    pub fn update_disabled(&self, is_disabled: bool) {
        self.add_update_message(UpdateMessage::Disabled {
            id: *self,
            is_disabled,
        });
    }

    pub fn request_paint(&self) {
        self.add_update_message(UpdateMessage::RequestPaint);
    }

    pub fn request_layout(&self) {
        self.add_update_message(UpdateMessage::RequestLayout { id: *self });
    }

    pub fn update_state(&self, state: impl Any, deferred: bool) {
        if let Some(root) = self.root_id() {
            if !deferred {
                UPDATE_MESSAGES.with(|msgs| {
                    let mut msgs = msgs.borrow_mut();
                    let msgs = msgs.entry(root).or_default();
                    msgs.push(UpdateMessage::State {
                        id: *self,
                        state: Box::new(state),
                    })
                });
            } else {
                DEFERRED_UPDATE_MESSAGES.with(|msgs| {
                    let mut msgs = msgs.borrow_mut();
                    let msgs = msgs.entry(root).or_default();
                    msgs.push((*self, Box::new(state)));
                });
            }
        }
    }

    pub fn update_base_style(&self, style: Style) {
        self.add_update_message(UpdateMessage::BaseStyle { id: *self, style });
    }

    pub fn update_style(&self, style: Style) {
        self.add_update_message(UpdateMessage::Style { id: *self, style });
    }

    pub fn update_style_selector(&self, style: Style, selector: StyleSelector) {
        self.add_update_message(UpdateMessage::StyleSelector {
            id: *self,
            style,
            selector,
        });
    }

    pub fn keyboard_navigatable(&self) {
        self.add_update_message(UpdateMessage::KeyboardNavigable { id: *self });
    }

    pub fn draggable(&self) {
        self.add_update_message(UpdateMessage::Draggable { id: *self });
    }

    pub fn update_responsive_style(&self, style: Style, size: ScreenSize) {
        self.add_update_message(UpdateMessage::ResponsiveStyle {
            id: *self,
            style,
            size,
        });
    }

    pub fn update_event_listener(&self, listener: EventListener, action: Box<EventCallback>) {
        self.add_update_message(UpdateMessage::EventListener {
            id: *self,
            listener,
            action,
        });
    }

    pub fn update_resize_listener(&self, action: Box<ResizeCallback>) {
        self.add_update_message(UpdateMessage::ResizeListener { id: *self, action });
    }

    pub fn update_cleanup_listener(&self, action: Box<dyn Fn()>) {
        self.add_update_message(UpdateMessage::CleanupListener { id: *self, action });
    }

    pub fn update_animation(&self, animation: Animation) {
        self.add_update_message(UpdateMessage::Animation {
            id: *self,
            animation,
        });
    }

    pub fn update_context_menu(&self, menu: Box<MenuCallback>) {
        self.add_update_message(UpdateMessage::ContextMenu { id: *self, menu });
    }

    pub fn update_popout_menu(&self, menu: Box<MenuCallback>) {
        self.add_update_message(UpdateMessage::PopoutMenu { id: *self, menu });
    }

    fn add_update_message(&self, msg: UpdateMessage) {
        if let Some(root) = self.root_id() {
            UPDATE_MESSAGES.with(|msgs| {
                let mut msgs = msgs.borrow_mut();
                let msgs = msgs.entry(root).or_default();
                msgs.push(msg);
            });
        }
    }
}
