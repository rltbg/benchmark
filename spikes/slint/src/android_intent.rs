//! Lecture de l'extra d'intent `autorun` (am start --es autorun 1) en JNI brut (crate jni-sys) :
//! activity.getIntent().getStringExtra("autorun").equals("1"). Aucune API Slint ne l'expose.
use jni_sys::{JNIEnv, JavaVM, jvalue};
use slint::android::AndroidApp;
use std::ffi::{CStr, c_void};
use std::sync::OnceLock;

static APP: OnceLock<AndroidApp> = OnceLock::new();

pub fn set_app(app: AndroidApp) {
    let _ = APP.set(app);
}

pub fn autorun_extra() -> bool {
    let Some(app) = APP.get() else { return false };
    unsafe { read_extra(app.vm_as_ptr() as *mut JavaVM, app.activity_as_ptr() as jni_sys::jobject) }.map(|v| v == "1").unwrap_or(false)
}

unsafe fn read_extra(vm: *mut JavaVM, activity: jni_sys::jobject) -> Option<String> {
    unsafe {
        let mut envp: *mut c_void = std::ptr::null_mut();
        if ((**vm).AttachCurrentThread)(vm, &mut envp, std::ptr::null_mut()) != 0 {
            return None;
        }
        let env = envp as *mut JNIEnv;
        let f = &**env;
        let act_cls = (f.GetObjectClass)(env, activity);
        let get_intent = (f.GetMethodID)(env, act_cls, c"getIntent".as_ptr(), c"()Landroid/content/Intent;".as_ptr());
        let intent = (f.CallObjectMethodA)(env, activity, get_intent, std::ptr::null());
        if intent.is_null() || (f.ExceptionCheck)(env) != 0 {
            (f.ExceptionClear)(env);
            return None;
        }
        let int_cls = (f.GetObjectClass)(env, intent);
        let get_extra = (f.GetMethodID)(env, int_cls, c"getStringExtra".as_ptr(), c"(Ljava/lang/String;)Ljava/lang/String;".as_ptr());
        let key = (f.NewStringUTF)(env, c"autorun".as_ptr());
        let args = [jvalue { l: key }];
        let val = (f.CallObjectMethodA)(env, intent, get_extra, args.as_ptr());
        if (f.ExceptionCheck)(env) != 0 {
            (f.ExceptionClear)(env);
            return None;
        }
        if val.is_null() {
            return None;
        }
        let chars = (f.GetStringUTFChars)(env, val, std::ptr::null_mut());
        let out = CStr::from_ptr(chars).to_string_lossy().into_owned();
        (f.ReleaseStringUTFChars)(env, val, chars);
        Some(out)
    }
}
