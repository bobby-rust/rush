extern crate glfw;
extern crate gl;
extern crate freetype;

use glfw::{Action, Context, Key};
use gl::types::*;
use std::mem;
use freetype::freetype::{FT_Face, FT_Library};
use std::{ffi::CString, ptr};
use std::fs;
use std::collections::HashMap;
use glam::Mat4;

#[derive(Debug, Clone, Copy)]
struct Character {
    texture_id: u32,
    size: (i32, i32),
    bearing: (i32, i32),
    advance: u32
}

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

fn load_font(characters: &mut HashMap<char, Character>) {
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

        freetype::freetype::FT_Set_Pixel_Sizes(face, 0, 48); 

        gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
        for c in 0..128 {
            if freetype::freetype::FT_Load_Char(face, c as u64, freetype::freetype::FT_LOAD_RENDER as i32) != 0 {
                panic!("Failed to load glyph");
            }
            let texture: *mut u32 = std::ptr::null_mut();
            gl::GenTextures(1, texture);
            gl::BindTexture(gl::TEXTURE_2D, *texture);

            let bitmap = (*face).glyph;
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RED as i32,
                (*bitmap).bitmap.width as i32,
                (*bitmap).bitmap.rows as i32,
                0, 
                gl::RED,
                gl::UNSIGNED_BYTE,
                (*bitmap).bitmap.buffer as *const std::ffi::c_void
            );

            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
            let character: Character = Character {
                texture_id: texture as u32,
                size: ((*bitmap).bitmap.width as i32, (*bitmap).bitmap.rows as i32),
                bearing: ((*bitmap).bitmap_left as i32, (*bitmap).bitmap_top as i32),
                advance: (*bitmap).advance.x as u32,
            };
            let ch = char::from_u32(c).expect("Failed to convert u32 to char");
            characters.insert(ch, character);
        }
        freetype::freetype::FT_Done_Face(face);
        freetype::freetype::FT_Done_FreeType(library);
    }
}

fn render_text() { // need to implement a shader
    // activate corresponding render state
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

    let mut characters: HashMap<char, Character> = HashMap::new();

    let mut glfw = glfw::init_no_callbacks().unwrap();
    let (mut window, events) = glfw.create_window(800, 600, "Rush Terminal", glfw::WindowMode::Windowed)
        .expect("Failed to create window");
    window.make_current();     
    window.set_key_polling(true);
    window.set_key_callback(key_callback);
    gl::load_with(|s| window.get_proc_address(s) as *const _);

    init_opengl(&glfw);
    unsafe {
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
    }

    let projection: Mat4 = glam::Mat4::orthographic_lh(0.0, 800.0, 0.0, 600.0, -1.0, 1.0);
    let mut vao: GLuint = 0;
    let mut vbo: GLuint = 0;

    unsafe {
        gl::GenVertexArrays(1, &mut vao);
        gl::GenBuffers(1, &mut vbo);

        // Bind the vao, vbo
        gl::BindVertexArray(vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
        
        // Allocate buffer memory without initializing data
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (mem::size_of::<f32>() * 6 * 4) as GLsizeiptr, // Buffer size
            ptr::null(), // No data initially 
            gl::DYNAMIC_DRAW // dynamic draw usage
        );
        
        // Enable the vertex attribute array
        gl::EnableVertexAttribArray(0);

        // Set the vertex attribute pointer
        gl::VertexAttribPointer(
            0, // attribute index
            4, // number of components per vertex (vec4)
            gl::FLOAT, // Data type
            gl::FALSE, // Normalize
            (4 * mem::size_of::<f32>()) as i32, // Stride (4 floats per vertex)
            ptr::null() // Offset (0 for the first attribute)
        );
       
        // Unbind the VBO
        gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        // Unbind the VAO
        gl::BindVertexArray(0);
    }

    load_font(&mut characters);
    let _program = create_shader_program();

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
