extern crate glfw;
extern crate gl;
extern crate freetype;

use glfw::{Action, Context, Key, Modifiers, Window};
use gl::types::*;
use freetype::freetype::{FT_Library, FT_Face};
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

fn compile_shader(source: &str, shader_type: GLenum) -> GLuint {
    let shader = unsafe { gl::CreateShader(shader_type) };
    let c_str = CString::new(source.as_bytes()).unwrap();
    unsafe {
        gl::ShaderSource(shader, 1, &c_str.as_ptr(), ptr::null());
        gl::CompileShader(shader);
        let mut success = 0;
        gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut success);
        if success == 0 {
            let mut info_log = vec![0; 512];
            gl::GetShaderInfoLog(shader, 512, ptr::null_mut(), info_log.as_mut_ptr() as *mut i8);
            println!("{:?}", info_log);
        }
        shader
    }
}

fn create_shader_program() -> GLuint {
    let vertex_shader = read_shader_file("/home/bobby/code/apps/rush/text.vert");
    let fragment_shader = read_shader_file("/home/bobby/code/apps/rush/text.frag");

    let vs = compile_shader(&vertex_shader, gl::VERTEX_SHADER);
    let fs = compile_shader(&fragment_shader, gl::FRAGMENT_SHADER);
    let program = unsafe { gl::CreateProgram() };
    let mut success = 0;
    unsafe {
        gl::GetProgramiv(program, gl::LINK_STATUS, &mut success);
        if success == 0 {
            let mut info_log = vec![0; 512];
            gl::GetProgramInfoLog(program, 512, ptr::null_mut(), info_log.as_mut_ptr() as *mut i8);
            gl::AttachShader(program, vs);
            gl::AttachShader(program, fs);
            gl::LinkProgram(program);
            gl::UseProgram(program);
        }
    }
    program
}

fn load_font() -> FT_Face {
    let font_path = CString::new("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf")
        .expect("CString::new failed."); // Default system font on Debian


    let mut library: FT_Library = std::ptr::null_mut(); 
    let mut face: FT_Face = std::ptr::null_mut();
    
    unsafe {
        if freetype::freetype::FT_Init_FreeType(&mut library) != 0 {
            panic!("Could not initialize freetype library");
        }

        let ft_log = freetype::freetype::FT_New_Face(library, font_path.as_ptr() as *const i8, 0, &mut face);

        if ft_log != 0 {
            println!("FT_New_Face return code: {:?}", ft_log);
            panic!("Failed to load font face");
        }

        if freetype::freetype::FT_Set_Char_Size(face, 48 * 64, 0, 96, 0) != 0 {
            panic!("Failed to set char size");
        }
    }

    face
}

fn check_gl_error() {
    unsafe {
        let error = gl::GetError();
        if error != gl::NO_ERROR {
            match error {
                gl::INVALID_ENUM => println!("OpenGL error: INVALID_ENUM (0x{:x})", error),
                gl::INVALID_VALUE => println!("OpenGL error: INVALID_VALUE (0x{:x})", error),
                gl::INVALID_OPERATION => println!("OpenGL error: INVALID_OPERATION (0x{:x})", error),
                gl::OUT_OF_MEMORY => println!("OpenGL error: OUT_OF_MEMORY (0x{:x})", error),
                _ => println!("OpenGL error: Unknown error (0x{:x})", error),
            }
        }
    }
}

fn render_text(text: &str, mut x: f32, y: f32, scale: f32, face: FT_Face) {
    let mut vao = 0;
    let mut vbo = 0;

    unsafe {
        gl::GenVertexArrays(1, &mut vao);
        gl::GenBuffers(1, &mut vbo);
        gl::BindVertexArray(vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
    }
    
    unsafe {
        for c in text.chars() {
            if freetype::freetype::FT_Load_Char(face, c as u64, freetype::freetype::FT_LOAD_RENDER as i32) == 0 {
                let bitmap = &*(*face).glyph;
                println!("{:?}", bitmap);
                let width = bitmap.metrics.width as i32;
                let height = bitmap.metrics.height as i32;
    
                // Create texture for the char bitmap
                let mut texture_id: GLuint = 0;
                gl::GenTextures(1, &mut texture_id);
                println!("Generated texture id: {:?}", texture_id);
                check_gl_error();

                gl::BindTexture(gl::TEXTURE_2D, texture_id);
                check_gl_error();

                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
                check_gl_error();                

                gl::TexImage2D(
                    gl::TEXTURE_2D,
                    0,
                    gl::RED as GLint,
                    width as GLint,
                    height as GLint,
                    0,
                    gl::RGBA,
                    gl::UNSIGNED_BYTE,
                    bitmap.bitmap.buffer as *const std::ffi::c_void,
                );
                check_gl_error();
                
                // Render the char at the specified position
                let vertices: [GLfloat; 20] = [
                    x, y, 0.0, 0.0, 1.0, // bottom left
                    x + scale, y, 1.0, 0.0, 1.0, // bottom right
                    x, y + scale, 0.0, 1.0, 1.0, // top left
                    x + scale, y + scale, 1.0, 1.0, 1.0, // top right
                ];

                gl::BufferData(
                    gl::ARRAY_BUFFER,
                    (vertices.len() * std::mem::size_of::<f32>()) as GLsizeiptr,
                    vertices.as_ptr() as *const std::ffi::c_void,
                    gl::STATIC_DRAW
                );


                // Set vertex attribute pointers
                gl::VertexAttribPointer(0, 2, gl::FLOAT, gl::FALSE, 5 * std::mem::size_of::<GLfloat>() as i32, std::ptr::null());
                gl::EnableVertexAttribArray(0);

                gl::VertexAttribPointer(1, 3, gl::FLOAT, gl::FALSE, 5 * std::mem::size_of::<GLfloat>() as i32, (2 * std::mem::size_of::<GLfloat>()) as *const std::ffi::c_void);
                gl::EnableVertexAttribArray(1);

                // Draw the quad for this char (VAO/VBO setup needed)
                gl::BindTexture(gl::TEXTURE_2D, texture_id);
                gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4);
                check_gl_error();
                x += scale;
            }
        }
    }
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

    init_opengl(&glfw);

    let face = load_font();
    let program = create_shader_program();

    unsafe {
        gl::ClearColor(0.0, 0.0, 0.0, 1.0);
    }

    while !window.should_close() {
        glfw.poll_events();
        unsafe {
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }
        
        render_text("Hello, world!", 100.0, 100.0, 1.0, face);
        window.swap_buffers();
    }
}
