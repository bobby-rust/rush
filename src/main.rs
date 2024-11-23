extern crate glfw;
extern crate gl;

use glfw::{Action, Context, Key, Modifiers, Window};
use gl::types::*;
use stb_truetype::FontInfo;
use std::{ffi::CString, ptr};
use std::fs;
use std::path::Path;

const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;

fn read_shader_file(path: &str) -> String {
    fs::read_to_string(path)
        .expect(&format!("Failed to read shader file: {}", path))
}

fn key_callback(window: &mut glfw::Window, key: Key, _scancode: i32, action: Action, _mods: glfw::Modifiers) {
    if action == Action::Press || action == Action::Repeat {
        // Capture key presses
        println!("Key pressed: {:?}", key);
    }
}

fn init_opengl(glfw: &glfw::Glfw) {
    gl::load_with(|s| glfw.get_proc_address_raw(s) as *const _);
}

fn main() {
    let ascii_art = r#"
                         ______  
    ___________  ___________  /_ 
    __  ___/  / / /_  ___/_  __ \
    _  /   / /_/ /_(__  )_  / / /
    /_/    \__,_/ /____/ /_/ /_/ 
        "#;

    println!("{}", ascii_art);

    let mut glfw = glfw::init_no_callbacks().unwrap();
    let (mut window, events) = glfw.create_window(800, 600, "Rush Terminal", glfw::WindowMode::Windowed)
        .expect("Failed to create window");
    window.make_current();     
    window.set_key_polling(true);
    window.set_key_callback(key_callback);
    gl::load_with(|s| window.get_proc_address(s) as *const _);

    unsafe {
        gl::ClearColor(0.0, 0.0, 0.0, 1.0);
    }

    while !window.should_close() {
        glfw.poll_events();
        unsafe {
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }

        window.swap_buffers();
    }
}
