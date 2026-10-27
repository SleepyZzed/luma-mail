//! Model-backed inbox. Only GtkListView factory bindings own GTK row widgets.
//! Selection and identity live in metadata, including rows outside the viewport.
use adw::prelude::*;
use gtk::{gio, glib};
use luma_core::model::{Conversation, ThreadKey};
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::{Rc, Weak},
};

type Compare = fn(&Row, &Row) -> gtk::Ordering;
struct Inner {
    store: gio::ListStore,
    selection: gtk::MultiSelection,
    mode: Cell<gtk::SelectionMode>,
    mutating: Cell<bool>,
    sort: Cell<Option<Compare>>,
    factory: RefCell<Option<gtk::SignalListItemFactory>>,
    suspended_scroll: Cell<Option<f64>>,
}
struct Data {
    item: RefCell<Conversation>,
    owner: Weak<Inner>,
    position: Cell<i32>,
    name: RefCell<String>,
    sensitive: Cell<bool>,
    classes: RefCell<HashSet<String>>,
    widget: glib::WeakRef<gtk::Box>,
    reveal: glib::WeakRef<gtk::Revealer>,
    visible: Cell<bool>,
}
#[derive(Clone)]
pub struct Row(Rc<Data>);
impl std::fmt::Debug for Row {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Row").field(&self.key()).finish()
    }
}
impl PartialEq for Row {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Row {}
#[derive(Clone)]
pub struct WeakRow(Weak<Data>);
impl WeakRow {
    pub fn upgrade(&self) -> Option<Row> {
        self.0.upgrade().map(Row)
    }
}
#[derive(Clone)]
pub struct Reveal(Row);
impl Reveal {
    pub fn set_child(&self, child: Option<&gtk::Box>) {
        if let Some(reveal) = self.0.0.reveal.upgrade() {
            reveal.set_child(child);
        }
    }
    pub fn set_reveal_child(&self, visible: bool) {
        self.0.0.visible.set(visible);
        if let Some(reveal) = self.0.0.reveal.upgrade() {
            reveal.set_reveal_child(visible);
        }
    }
}
impl Row {
    pub fn new(list: &Inbox, item: Conversation) -> Self {
        Self(Rc::new(Data {
            item: RefCell::new(item),
            owner: Rc::downgrade(&list.inner),
            position: Cell::new(-1),
            name: RefCell::new(String::new()),
            sensitive: Cell::new(true),
            classes: Default::default(),
            widget: Default::default(),
            reveal: Default::default(),
            visible: Cell::new(true),
        }))
    }
    pub fn key(&self) -> ThreadKey {
        self.0.item.borrow().key.clone()
    }
    pub fn item(&self) -> Conversation {
        self.0.item.borrow().clone()
    }
    pub fn set_item(&self, item: Conversation) {
        *self.0.item.borrow_mut() = item;
    }
    pub fn reveal(&self) -> Reveal {
        Reveal(self.clone())
    }
    pub fn widget(&self) -> Option<gtk::Box> {
        self.0.widget.upgrade()
    }
    pub fn bind(&self, widget: &gtk::Box, reveal: &gtk::Revealer) {
        self.0.widget.set(Some(widget));
        self.0.reveal.set(Some(reveal));
        widget.set_sensitive(self.0.sensitive.get());
        for class in self.0.classes.borrow().iter() {
            widget.add_css_class(class);
        }
        reveal.set_reveal_child(self.0.visible.get());
    }
    pub fn unbind(&self) {
        self.0.widget.set(None::<&gtk::Box>);
        self.0.reveal.set(None::<&gtk::Revealer>);
    }
    pub fn downgrade(&self) -> WeakRow {
        WeakRow(Rc::downgrade(&self.0))
    }
    pub fn index(&self) -> i32 {
        self.0.position.get()
    }
    pub fn is_selected(&self) -> bool {
        self.index() >= 0
            && self
                .0
                .owner
                .upgrade()
                .is_some_and(|i| i.selection.is_selected(self.index() as u32))
    }
    pub fn is_sensitive(&self) -> bool {
        self.0.sensitive.get()
    }
    pub fn set_sensitive(&self, value: bool) {
        self.0.sensitive.set(value);
        if let Some(w) = self.widget() {
            w.set_sensitive(value);
        }
    }
    pub fn set_selectable(&self, value: bool) {
        self.set_sensitive(value);
    }
    pub fn add_css_class(&self, class: &str) {
        self.0.classes.borrow_mut().insert(class.into());
        if let Some(w) = self.widget() {
            w.add_css_class(class);
        }
    }
    pub fn remove_css_class(&self, class: &str) {
        self.0.classes.borrow_mut().remove(class);
        if let Some(w) = self.widget() {
            w.remove_css_class(class);
        }
    }
    pub fn has_css_class(&self, class: &str) -> bool {
        self.0.classes.borrow().contains(class)
    }
    pub fn set_widget_name(&self, name: &str) {
        *self.0.name.borrow_mut() = name.into();
    }
    pub fn widget_name(&self) -> String {
        self.0.name.borrow().clone()
    }
    pub fn changed(&self) {
        if let Some(inner) = self.0.owner.upgrade() {
            inner.resort(Some(self));
        }
    }
    pub fn compute_bounds(&self, target: &gtk::ListView) -> Option<gtk::graphene::Rect> {
        let r = self.widget()?.compute_bounds(target)?;
        let offset = target.vadjustment().map_or(0., |a| a.value()) as f32;
        Some(gtk::graphene::Rect::new(
            r.x(),
            r.y() + offset,
            r.width(),
            r.height(),
        ))
    }
    #[cfg(debug_assertions)]
    pub fn last_child(&self) -> Option<gtk::Widget> {
        self.widget()?.last_child()
    }
}
impl Inner {
    fn rows(&self) -> Vec<Row> {
        (0..self.store.n_items())
            .filter_map(|n| self.row(n))
            .collect()
    }
    fn row(&self, n: u32) -> Option<Row> {
        self.store
            .item(n)?
            .downcast::<glib::BoxedAnyObject>()
            .ok()
            .map(|o| o.borrow::<Row>().clone())
    }
    fn selected(&self) -> Vec<Row> {
        let selected = self.selection.selection();
        let Some((iter, first)) = gtk::BitsetIter::init_first(&selected) else {
            return Vec::new();
        };
        std::iter::once(first)
            .chain(iter)
            .filter_map(|n| self.row(n))
            .collect()
    }
    fn resort(&self, changed: Option<&Row>) {
        let selected: HashSet<_> = self.selected().iter().map(Row::key).collect();
        let mut rows = self.rows();
        if let Some(sort) = self.sort.get() {
            rows.sort_by(|a, b| match sort(a, b) {
                gtk::Ordering::Smaller => std::cmp::Ordering::Less,
                gtk::Ordering::Larger => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            });
        }
        let old = self.rows();
        self.mutating.set(true);
        // Keep stable model objects outside the changed range. Selection is
        // restored by account/thread identity, never by recycled widget index.
        let first =
            (0..rows.len()).find(|&n| old[n] != rows[n] || changed.is_some_and(|r| r == &rows[n]));
        if let Some(first) = first {
            let last = (first..rows.len())
                .rev()
                .find(|&n| old[n] != rows[n] || changed.is_some_and(|r| r == &rows[n]))
                .unwrap();
            let objects: Vec<_> = rows[first..=last]
                .iter()
                .cloned()
                .map(glib::BoxedAnyObject::new)
                .collect();
            self.store
                .splice(first as u32, (last - first + 1) as u32, &objects);
        }
        for (n, row) in rows.iter().enumerate() {
            row.0.position.set(n as i32);
        }
        self.selection.unselect_all();
        for row in rows
            .iter()
            .filter(|r| r.is_sensitive() && selected.contains(&r.key()))
        {
            self.selection.select_item(row.index() as u32, false);
        }
        self.mutating.set(false);
    }
}
#[derive(Clone)]
pub struct Inbox {
    pub widget: gtk::ListView,
    inner: Rc<Inner>,
}
impl std::ops::Deref for Inbox {
    type Target = gtk::ListView;
    fn deref(&self) -> &Self::Target {
        &self.widget
    }
}
impl Default for Inbox {
    fn default() -> Self {
        Self::new()
    }
}
impl Inbox {
    pub fn new() -> Self {
        let store = gio::ListStore::new::<glib::BoxedAnyObject>();
        let selection = gtk::MultiSelection::new(Some(store.clone()));
        let widget = gtk::ListView::new(Some(selection.clone()), None::<gtk::ListItemFactory>);
        widget.set_single_click_activate(false);
        Self {
            widget,
            inner: Rc::new(Inner {
                store,
                selection,
                mode: Cell::new(gtk::SelectionMode::Single),
                mutating: Cell::new(false),
                sort: Cell::new(None),
                factory: RefCell::new(None),
                suspended_scroll: Cell::new(None),
            }),
        }
    }
    pub fn install(&self, bind: impl Fn(&Row, &gtk::Box, &gtk::Revealer) + 'static) {
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_bind(move |_, object| {
            let item = object.downcast_ref::<gtk::ListItem>().unwrap();
            let row = item
                .item()
                .unwrap()
                .downcast::<glib::BoxedAnyObject>()
                .unwrap()
                .borrow::<Row>()
                .clone();
            let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let reveal = gtk::Revealer::new();
            widget.append(&reveal);
            row.bind(&widget, &reveal);
            bind(&row, &widget, &reveal);
            item.set_selectable(row.is_sensitive());
            item.set_activatable(row.is_sensitive());
            item.set_child(Some(&widget));
        });
        factory.connect_unbind(|_, object| {
            let item = object.downcast_ref::<gtk::ListItem>().unwrap();
            if let Some(object) = item.item().and_downcast::<glib::BoxedAnyObject>() {
                object.borrow::<Row>().unbind();
            }
            if let Some(child) = item.child() {
                while let Some(pop) = child.last_child().and_downcast::<gtk::Popover>() {
                    pop.popdown();
                    pop.unparent();
                }
            }
            item.set_child(None::<&gtk::Widget>);
        });
        self.widget.set_factory(Some(&factory));
        *self.inner.factory.borrow_mut() = Some(factory);
    }
    pub fn suspend(&self) {
        if self.inner.suspended_scroll.get().is_some() {
            return;
        }
        self.inner
            .suspended_scroll
            .set(Some(self.widget.vadjustment().map_or(0., |a| a.value())));
        self.widget.set_factory(None::<&gtk::ListItemFactory>);
    }
    pub fn resume(&self) {
        let Some(scroll) = self.inner.suspended_scroll.take() else {
            return;
        };
        self.widget
            .set_factory(self.inner.factory.borrow().as_ref());
        let ticks = Cell::new(0);
        // Four allocation frames only, when returning from the tray. No idle
        // timer or persistent frame callback is needed for viewport restoration.
        self.widget.add_tick_callback(move |widget, _| {
            if let Some(a) = widget.vadjustment() {
                a.set_value(scroll.min((a.upper() - a.page_size()).max(0.)));
            }
            ticks.set(ticks.get() + 1);
            if ticks.get() >= 4 {
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }
    pub fn set_sort_func(&self, sort: fn(&Row, &Row) -> gtk::Ordering) {
        self.inner.sort.set(Some(sort));
    }
    pub fn selection_mode(&self) -> gtk::SelectionMode {
        self.inner.mode.get()
    }
    pub fn set_selection_mode(&self, mode: gtk::SelectionMode) {
        self.inner.mode.set(mode);
    }
    pub fn selected_rows(&self) -> Vec<Row> {
        self.inner.selected()
    }
    pub fn unselect_all(&self) {
        self.inner.selection.unselect_all();
    }
    pub fn select_row(&self, row: Option<&Row>) {
        if let Some(row) = row.filter(|r| r.index() >= 0 && r.is_sensitive()) {
            self.inner.selection.select_item(
                row.index() as u32,
                self.selection_mode() != gtk::SelectionMode::Multiple,
            );
        }
    }
    pub fn unselect_row(&self, row: &Row) {
        if row.index() >= 0 {
            self.inner.selection.unselect_item(row.index() as u32);
        }
    }
    pub fn row_at_index(&self, n: i32) -> Option<Row> {
        if n < 0 {
            None
        } else {
            self.inner.row(n as u32)
        }
    }
    pub fn row_at_y(&self, y: i32) -> Option<Row> {
        self.inner.rows().into_iter().find(|r| {
            r.compute_bounds(&self.widget)
                .is_some_and(|b| y as f32 >= b.y() && (y as f32) < b.y() + b.height())
        })
    }
    pub fn append(&self, row: &Row) {
        self.insert(row, self.inner.store.n_items() as i32);
    }
    pub fn append_many(&self, rows: &[Row]) {
        if rows.is_empty() {
            return;
        }
        self.inner.mutating.set(true);
        let objects: Vec<_> = rows
            .iter()
            .cloned()
            .map(glib::BoxedAnyObject::new)
            .collect();
        self.inner
            .store
            .splice(self.inner.store.n_items(), 0, &objects);
        for (n, row) in self.inner.rows().iter().enumerate() {
            row.0.position.set(n as i32);
        }
        self.inner.mutating.set(false);
        self.inner.resort(None);
    }
    pub fn insert(&self, row: &Row, n: i32) {
        let n = (n as u32).min(self.inner.store.n_items());
        self.inner.mutating.set(true);
        self.inner
            .store
            .insert(n, &glib::BoxedAnyObject::new(row.clone()));
        for (n, row) in self.inner.rows().iter().enumerate() {
            row.0.position.set(n as i32);
        }
        self.inner.mutating.set(false);
        self.inner.resort(None);
    }
    pub fn remove(&self, row: &Row) {
        if row.index() < 0 {
            return;
        }
        self.inner.mutating.set(true);
        self.inner.store.remove(row.index() as u32);
        row.0.position.set(-1);
        for (n, row) in self.inner.rows().iter().enumerate() {
            row.0.position.set(n as i32);
        }
        self.inner.mutating.set(false);
    }
    pub fn clear(&self) {
        self.inner.mutating.set(true);
        for row in self.inner.rows() {
            row.0.position.set(-1);
        }
        self.inner.store.remove_all();
        self.inner.mutating.set(false);
    }
    pub fn connect_selected_rows_changed(&self, call: impl Fn(&Self) + 'static) {
        let weak = Rc::downgrade(&self.inner);
        let widget = self.widget.downgrade();
        self.inner
            .selection
            .connect_selection_changed(move |_, _, _| {
                if let (Some(inner), Some(widget)) = (weak.upgrade(), widget.upgrade())
                    && !inner.mutating.get()
                {
                    call(&Self { widget, inner });
                }
            });
    }
    pub fn connect_row_selected(&self, call: impl Fn(&Self, Option<&Row>) + 'static) {
        self.connect_selected_rows_changed(move |list| {
            let selected = list.selected_rows();
            call(list, selected.last());
        });
    }
    pub fn connect_row_activated(&self, call: impl Fn(&Self, &Row) + 'static) {
        let weak = Rc::downgrade(&self.inner);
        self.widget.connect_activate(move |widget, n| {
            if let Some(inner) = weak.upgrade()
                && let Some(row) = inner.row(n)
            {
                call(
                    &Self {
                        widget: widget.clone(),
                        inner,
                    },
                    &row,
                );
            }
        });
    }
    #[cfg(any(debug_assertions, target_os = "linux", target_os = "windows"))]
    pub fn bound_count(&self) -> usize {
        self.inner
            .rows()
            .iter()
            .filter(|r| r.widget().is_some())
            .count()
    }
}
