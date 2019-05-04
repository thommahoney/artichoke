use std::collections::HashSet;
use std::convert::AsRef;
use std::ffi::CString;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use crate::def::{ClassLike, Define, Free, Method, Parent};
use crate::interpreter::{Mrb, MrbError};
use crate::method;
use crate::sys;

pub struct Spec {
    name: String,
    cstring: CString,
    data_type: sys::mrb_data_type,
    methods: HashSet<method::Spec>,
    parent: Option<Parent>,
    is_mrb_tt_data: bool,
}

impl Spec {
    pub fn new<T>(name: T, parent: Option<Parent>, free: Option<Free>) -> Self
    where
        T: AsRef<str>,
    {
        let cstr = CString::new(name.as_ref()).expect("name for data type");
        let data_type = sys::mrb_data_type {
            struct_name: cstr.as_ptr(),
            dfree: free,
        };
        Self {
            name: name.as_ref().to_owned(),
            cstring: cstr,
            data_type,
            methods: HashSet::new(),
            parent,
            is_mrb_tt_data: false,
        }
    }

    pub fn data_type(&self) -> &sys::mrb_data_type {
        &self.data_type
    }

    pub fn mrb_value_is_rust_backed(&mut self, is_mrb_tt_data: bool) {
        self.is_mrb_tt_data = is_mrb_tt_data;
    }
}

impl ClassLike for Spec {
    fn add_method(&mut self, name: &str, method: Method, args: sys::mrb_aspec) {
        let spec = method::Spec::new(method::Type::Instance, name, method, args);
        self.methods.insert(spec);
    }

    fn add_self_method(&mut self, name: &str, method: Method, args: sys::mrb_aspec) {
        let spec = method::Spec::new(method::Type::Class, name, method, args);
        self.methods.insert(spec);
    }

    fn cstring(&self) -> &CString {
        &self.cstring
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn parent(&self) -> Option<Parent> {
        self.parent.clone()
    }

    fn rclass(&self, interp: Mrb) -> *mut sys::RClass {
        let mrb = interp.borrow().mrb;
        if let Some(ref parent) = self.parent {
            unsafe {
                sys::mrb_class_get_under(mrb, (*parent).rclass(interp), self.cstring().as_ptr())
            }
        } else {
            unsafe { sys::mrb_class_get(mrb, self.cstring().as_ptr()) }
        }
    }
}

impl fmt::Debug for Spec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)?;
        if self.data_type.dfree.is_some() {
            write!(f, " -- with free func")?;
        }
        Ok(())
    }
}

impl fmt::Display for Spec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "mruby class spec -- {}", self.name)
    }
}

impl Hash for Spec {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name().hash(state);
        self.parent().hash(state);
    }
}

impl Eq for Spec {}

impl PartialEq for Spec {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Define for Spec {
    fn define(&self, interp: Mrb) -> Result<*mut sys::RClass, MrbError> {
        let mrb = interp.borrow().mrb;
        let rclass = if let Some(ref parent) = self.parent {
            unsafe {
                sys::mrb_define_class_under(
                    mrb,
                    parent.rclass(Rc::clone(&interp)),
                    self.cstring().as_ptr(),
                    (*mrb).object_class,
                )
            }
        } else {
            unsafe { sys::mrb_define_class(mrb, self.cstring().as_ptr(), (*mrb).object_class) }
        };
        for method in self.methods.iter() {
            method.define(Rc::clone(&interp), rclass)?;
        }
        // If a `Spec` defines a `Class` whose isntances own a pointer to a
        // Rust object, mark them as `MRB_TT_DATA`.
        if self.is_mrb_tt_data {
            unsafe {
                sys::mrb_sys_set_instance_tt(rclass, sys::mrb_vtype::MRB_TT_DATA);
            }
        }
        Ok(rclass)
    }
}
