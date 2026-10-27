//! A one-child GTK container around NSH's widget whose only job is to know
//! whether a frame redrew anything: GTK calls a widget's `snapshot` again only
//! when it or a descendant was invalidated (`gtk_widget_queue_draw` invalidates
//! every ancestor's render node), so a frame whose `snapshot` did not run here
//! changed no pixel and is not counted. Public GTK subclassing API only.

use std::cell::Cell;

use nsh_platform_linux::gtk4::{self as gtk, glib, prelude::*, subclass::prelude::*};

thread_local! {
    pub static DREW: Cell<bool> = const { Cell::new(false) };
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Probe;

    #[glib::object_subclass]
    impl ObjectSubclass for Probe {
        const NAME: &'static str = "NshBenchProbe";
        type Type = super::Probe;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for Probe {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Probe {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            DREW.with(|d| d.set(true));
            self.parent_snapshot(snapshot);
        }
    }
}

glib::wrapper! {
    pub struct Probe(ObjectSubclass<imp::Probe>) @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Probe {
    pub fn new(child: &gtk::Widget) -> Self {
        let p: Self = glib::Object::new();
        p.set_hexpand(true);
        p.set_vexpand(true);
        child.set_parent(&p);
        p
    }
}

/// Whether anything was redrawn since the last call.
pub fn take() -> bool {
    DREW.with(|d| d.replace(false))
}
