extern crate gl;
extern crate glfw;
extern crate gl_loader;

use glfw::{Action, Context, Key};
use gl::types::*;
use std::os::raw::c_void;

const WINDOW_WIDTH:  u32 = 800;
const WINDOW_HEIGHT: u32 = 600;

fn init_opengl() {
    gl_loader::init_gl();
    gl::load_with(|symbol| gl_loader::get_proc_address(symbol) as *const _);
}

fn framebuffer_size_callback(window: &mut glfw::Window, width: i32, height: i32) {
    println!("Framebuffer size callback called with args {:?} {:?}", width, height);
    unsafe {
        gl::Viewport(0, 0, width, height);
    }
}

fn check_shader_compilation(shader: u32, check_type: u32) {
    let mut success = gl::FALSE as gl::types::GLint;
    let mut info_log = vec![0u8; 512];

    unsafe {
        // Check compile status
        gl::GetShaderiv(shader, check_type, &mut success);
        if success == gl::FALSE as gl::types::GLint {
            // Retrieve error log
            gl::GetShaderInfoLog(
                shader,
                info_log.len() as gl::types::GLsizei,
                std::ptr::null_mut(),
                info_log.as_mut_ptr() as *mut gl::types::GLchar,
            );

            // Convert error log to a Rust string
            let error_message = std::ffi::CStr::from_ptr(info_log.as_ptr() as *const gl::types::GLchar)
                .to_string_lossy()
                .into_owned();

            // Print error message
            eprintln!("ERROR::SHADER::COMPILATION_FAILED\n{}", error_message);
        }
    }
}

fn main() {
    let mut glfw = glfw::init_no_callbacks().unwrap();
    let (mut window, events) = glfw.create_window(
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        "rush", 
        glfw::WindowMode::Windowed)
        .expect("Failed to create window.");
    
    glfw::Window::set_framebuffer_size_callback(&mut window, framebuffer_size_callback);

    // Make the window's context current
    window.make_current();
    window.set_key_polling(true);

    init_opengl();

    unsafe {
        gl::Viewport(0, 0, WINDOW_WIDTH as i32, WINDOW_HEIGHT as i32);
    }

    
    let vertex_shader_source = std::ffi::CString::new(r#"
        #version 330 core
        layout (location = 0) in vec3 aPos;

        void main() {
            gl_Position = vec4(aPos.x, aPos.y, aPos.z, 1.0);
        }"#).unwrap();

    let fragment_shader_source = std::ffi::CString::new(r#"
        #version 330 core
        out vec4 FragColor;

        void main() {
            FragColor = vec4(1.0f, 1.0f, 0.2f, 1.0f);
        }
        "#).unwrap();
 
    let mut vbo: u32 = 0;
    let mut vao: u32 = 0;      
    let shader_program: u32 = 0;
    unsafe {
        // Compile vertex shader
        let vertex_shader = gl::CreateShader(gl::VERTEX_SHADER);
        gl::ShaderSource(vertex_shader, 1, &vertex_shader_source.as_ptr(), std::ptr::null());
        gl::CompileShader(vertex_shader);
        check_shader_compilation(vertex_shader, gl::COMPILE_STATUS);

        // Compile fragment shader
        let fragment_shader = gl::CreateShader(gl::FRAGMENT_SHADER);
        gl::ShaderSource(fragment_shader, 1, &fragment_shader_source.as_ptr(), std::ptr::null());
        gl::CompileShader(fragment_shader);
        check_shader_compilation(fragment_shader, gl::COMPILE_STATUS);
        
        // Link shaders and create shader program
        let shader_program: u32 = gl::CreateProgram();
        gl::AttachShader(shader_program, vertex_shader);
        gl::AttachShader(shader_program, fragment_shader);
        gl::LinkProgram(shader_program);
        // TODO: Should check link status here...

        gl::DeleteShader(vertex_shader);
        gl::DeleteShader(fragment_shader);

        let vertices: [f32; 9] = [
            -0.5, -0.5, 0.0,
             0.5, -0.5, 0.0,
             0.0,  0.5, 0.0,
        ];


        gl::GenVertexArrays(1, &mut vao);
        gl::GenBuffers(1, &mut vbo);
        gl::BindVertexArray(vao);

        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER, 
            (std::mem::size_of::<f32>() * vertices.len()) as isize,
            vertices.as_ptr() as *const c_void,
            gl::STATIC_DRAW
        );

        gl::VertexAttribPointer(0, 3, gl::FLOAT, gl::FALSE, 3 * std::mem::size_of::<f32>() as i32, std::ptr::null());
        gl::EnableVertexAttribArray(0);
        
        gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        gl::BindVertexArray(0);
    }



    // Loop until the user closes the window
    while !window.should_close() {
        window.swap_buffers();

        glfw.poll_events();
        for (_, event) in glfw::flush_messages(&events) {
            println!("{:?}", event);
            match event {
                glfw::WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                    window.set_should_close(true);
                },
                _ => {}
            }
        }

        unsafe {
            gl::ClearColor(0.2, 0.3, 0.3, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            gl::UseProgram(shader_program);

            gl::BindVertexArray(vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 3);
        }    
    }
}
