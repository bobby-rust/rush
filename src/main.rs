mod shader;

extern crate gl;
extern crate glfw;
extern crate gl_loader;

use glfw::{Action, Context, Key};
use gl::types::*;
use std::os::raw::c_void;
use std::fs;

const WINDOW_WIDTH:  u16 = 800;
const WINDOW_HEIGHT: u16 = 600;
const NUM_VERTEX_ATTRIBS_RECT: u8 = 24;
const NUM_INDICES_RECT: u8 = 6;

fn init_opengl() {
    shader::hey();
    gl_loader::init_gl();
    gl::load_with(|symbol| gl_loader::get_proc_address(symbol) as *const _);
}

fn framebuffer_size_callback(_window: &mut glfw::Window, width: i32, height: i32) {
    println!("Framebuffer size callback called with args {:?} {:?}", width, height);
    unsafe {
        gl::Viewport(0, 0, width.into(), height.into());
    }
}

fn check_shader_link_status(shader: u32) {
    let mut success = gl::FALSE as GLint;
    let mut info_log = vec![0u8; 512];
    unsafe {
        gl::GetProgramiv(shader, gl::LINK_STATUS, &mut success);
        if success == gl::FALSE as GLint {
            gl::GetProgramInfoLog(
                shader,
                info_log.len() as GLsizei,
                std::ptr::null_mut(),
                info_log.as_mut_ptr() as *mut GLchar,
            );
            let error_message = std::ffi::CStr::from_ptr(info_log.as_ptr() as *const _)
                .to_string_lossy()
                .into_owned();
            eprintln!("ERROR::PROGRAM::LINKING_FAILED\n{}", error_message);
        }
    } 
}

fn check_shader_compile_status(shader: u32) {
    let mut success = gl::FALSE as GLint;
    let mut info_log =  vec![0u8; 512];

    unsafe {
        // Check compile status
        gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut success);
        if success == gl::FALSE as GLint {
            // Retrieve error log
            gl::GetShaderInfoLog(
                shader,
                info_log.len() as GLsizei,
                std::ptr::null_mut(),
                info_log.as_mut_ptr() as *mut GLchar,
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

fn load_object_into_mem(vertices: [f32; NUM_VERTEX_ATTRIBS_RECT as usize], indices: [u32; NUM_INDICES_RECT as usize]) -> (u32, u32) {
    let mut vao: u32 = 0;
    let mut vbo: u32 = 0;
    let mut ebo: u32 = 0;
   
    unsafe {
        // Generate and bind vao, vbo
        gl::GenVertexArrays(1, &mut vao);
        gl::BindVertexArray(vao);
        gl::GenBuffers(1, &mut vbo);
        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
        
        // Copy vertices data into vbo
        gl::BufferData(gl::ARRAY_BUFFER, (std::mem::size_of::<f32>() * vertices.len()).try_into().unwrap(), vertices.as_ptr() as *const c_void, gl::STATIC_DRAW);
        
        // generate and bind ebo
        gl::GenBuffers(1, &mut ebo);
        gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ebo);

        // Copy indices data into ebo
        gl::BufferData(gl::ELEMENT_ARRAY_BUFFER, (std::mem::size_of::<u32>() * indices.len()).try_into().unwrap(), indices.as_ptr() as *const c_void, gl::STATIC_DRAW);
        
        // Configure vertex attributes
        // position attrib
        gl::VertexAttribPointer(0, 3, gl::FLOAT, gl::FALSE, (6 * std::mem::size_of::<f32>()).try_into().unwrap(), std::ptr::null());
        gl::EnableVertexAttribArray(0);
        // Color attrib
        gl::VertexAttribPointer(1, 3, gl::FLOAT, gl::FALSE, (6 * std::mem::size_of::<f32>()).try_into().unwrap(), (3 * std::mem::size_of::<f32>()) as *const c_void);
        gl::EnableVertexAttribArray(1);

        gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        gl::BindVertexArray(0);
    }

    (vao, ebo)
}

unsafe fn create_shader_program(vertex_shader_path: &str, fragment_shader_path: &str) -> u32 {
    let shader_program: u32;

    let vertex_shader_source = fs::read_to_string(vertex_shader_path)
        .expect("Failed to read vertex shader source");
    let fragment_shader_source = fs::read_to_string(fragment_shader_path)
        .expect("Failed to read fragment shader source");

    let vertex_shader_cstr = std::ffi::CString::new(vertex_shader_source)
            .expect("Failed to create vertex shader CString");
    let fragment_shader_cstr = std::ffi::CString::new(fragment_shader_source)
            .expect("Failed to create fragment shader CString");

    // Compile vertex shader
    let vertex_shader = gl::CreateShader(gl::VERTEX_SHADER);
    gl::ShaderSource(vertex_shader, 1, &vertex_shader_cstr.as_ptr(), std::ptr::null());
    gl::CompileShader(vertex_shader);
    check_shader_compile_status(vertex_shader);

    // Compile fragment shader
    let fragment_shader = gl::CreateShader(gl::FRAGMENT_SHADER);
    gl::ShaderSource(fragment_shader, 1, &fragment_shader_cstr.as_ptr(), std::ptr::null());
    gl::CompileShader(fragment_shader);
    check_shader_compile_status(fragment_shader);
    
    // Link shaders and create shader program
    shader_program = gl::CreateProgram();
    gl::AttachShader(shader_program, vertex_shader);
    gl::AttachShader(shader_program, fragment_shader);
    gl::LinkProgram(shader_program);
    check_shader_link_status(shader_program);
    
    // Cleanup
    gl::DeleteShader(vertex_shader);
    gl::DeleteShader(fragment_shader);
    
    shader_program
}

unsafe fn draw_object_from_mem(vao: u32, ebo: u32) {
    gl::BindVertexArray(vao);
    gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ebo);
    gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_INT, std::ptr::null());
    gl::BindVertexArray(0);
}

fn main() {
    let mut glfw = glfw::init_no_callbacks().unwrap();
    let (mut window, events) = glfw.create_window(
        WINDOW_WIDTH.into(),
        WINDOW_HEIGHT.into(),
        "rush", 
        glfw::WindowMode::Windowed
    ).expect("Failed to create window.");
    
    glfw::Window::set_framebuffer_size_callback(&mut window, framebuffer_size_callback);

    // Make the window's context current
    window.make_current();
    window.set_key_polling(true);

    init_opengl();

    unsafe {
        gl::Viewport(0, 0, WINDOW_WIDTH as i32, WINDOW_HEIGHT as i32);
    }

    let rect_one_vertex_attribs: [f32; NUM_VERTEX_ATTRIBS_RECT as usize] = [
        // positions     // colors
        -1.0,  0.0, 0.0, 1.0, 0.0, 0.0, // bottom left
         0.0,  0.0, 0.0, 0.0, 1.0, 0.0, // bottom right
        -1.0,  1.0, 0.0, 0.0, 0.0, 1.0, // top left
         0.0,  1.0, 0.0, 0.0, 1.0, 0.0, // top right 
    ];

    let rect_two_vertex_attribs: [f32; NUM_VERTEX_ATTRIBS_RECT as usize] = [
        // Positions    // Colors
        0.0, -1.0, 0.0, 0.0, 0.0, 1.0, // bottom left
        1.0, -1.0, 0.0, 0.0, 1.0, 0.0, // bottom right
        0.0,  0.0, 0.0, 0.0, 1.0, 0.0, // top left
        1.0,  0.0, 0.0, 1.0, 0.0, 0.0 // top right
    ];

    let rect_one_indices: [u32; 6] = [
        0, 1, 2,
        1, 2, 3
    ];

    let rect_two_indices: [u32; 6] = [
        0, 1, 2,
        1, 2, 3
    ];
    
    let (vao1, ebo1) = load_object_into_mem(rect_one_vertex_attribs, rect_one_indices);
    let (vao2, ebo2) = load_object_into_mem(rect_two_vertex_attribs, rect_two_indices);
    
    let vertex_shader_path = "/home/bobby/code/apps/rush/vertex.vert";
    let fragment_shader_path = "/home/bobby/code/apps/rush/fragment.frag";
    let shader_program: u32 = unsafe { create_shader_program(vertex_shader_path, fragment_shader_path) };

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
            let dt = glfw.get_time();
            println!("{:?}", dt.sin() / 2.0 + 0.5);
            let red: f64 = dt.sin() / 2.0 + 0.5;
            let green: f64 = dt.sin() / 2.0 + 0.5;
            let blue: f64 = dt.sin() / 2.0 + 0.5;
            let color_uniform_name = std::ffi::CString::new("color").unwrap();
            let vertex_color_location = gl::GetUniformLocation(shader_program, color_uniform_name.as_ptr());
            gl::Uniform4f(vertex_color_location, red as f32, green as f32, blue as f32, 1.0);
             
            draw_object_from_mem(vao1, ebo1);           
            draw_object_from_mem(vao2, ebo2);           
        }    
    }
}
