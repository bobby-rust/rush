mod shader;

extern crate freetype;
extern crate gl;
extern crate gl_loader;
extern crate glfw;

use freetype::freetype as ft;
use glfw::{Action, Context, Key};
use shader::Shader;
use std::collections::HashMap;
use std::env;
use std::os::raw::c_void;

const WINDOW_WIDTH: u16 = 800;
const WINDOW_HEIGHT: u16 = 600;
const NUM_VERTEX_ATTRIBS_RECT: u8 = 24;
const NUM_INDICES_RECT: u8 = 6;

struct Character {
    texture_id: u32,
    size: (i32, i32),
    bearing: (i32, i32),
    advance: i64,
}

fn init_freetype_library() -> ft::FT_Library {
    let mut lib: ft::FT_Library = std::ptr::null_mut();
    unsafe {
        let err = ft::FT_Init_FreeType(&mut lib);
        if err != 0 {
            panic!(
                "Could not initialize FreeType library. ERROR CODE {:?}",
                lib
            );
        }
    }

    lib
}

fn create_ft_face(lib: ft::FT_Library, font_path: &std::ffi::CStr) -> ft::FT_Face {
    let face: ft::FT_Face = std::ptr::null_mut();
    let error = unsafe { ft::FT_New_Face(lib, font_path.as_ptr(), 0, face as *mut _) };
    if error != 0 {
        panic!("Could not create font face. ERROR CODE: {:?}", error);
    }

    face
}

fn render_text(lib: ft::FT_Library, face: ft::FT_Face) {
    let mut characters: HashMap<char, Character> = HashMap::new();
    unsafe {
        ft::FT_Set_Pixel_Sizes(face, 0, 48);

        gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);

        for c in 0..127 {
            let error = ft::FT_Load_Char(face, c, ft::FT_LOAD_RENDER as i32);
            if error != 0 {
                panic!("Could not load character. ERROR CODE: {:?}", error);
            }

            // Generate texture
            let mut texture: u32 = 0;
            let glyph = &*(*face).glyph;
            gl::GenTextures(1, &mut texture);
            gl::BindTexture(gl::TEXTURE_2D, texture);
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RED.try_into().unwrap(),
                glyph.bitmap.width.try_into().unwrap(),
                glyph.bitmap.rows.try_into().unwrap(),
                0,
                gl::RED,
                gl::UNSIGNED_BYTE,
                glyph.bitmap.buffer as *const _,
            );

            // Set texture options
            gl::TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_WRAP_S,
                gl::CLAMP_TO_EDGE.try_into().unwrap(),
            );
            gl::TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_WRAP_T,
                gl::CLAMP_TO_EDGE.try_into().unwrap(),
            );
            gl::TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_MIN_FILTER,
                gl::LINEAR.try_into().unwrap(),
            );
            gl::TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_MAG_FILTER,
                gl::LINEAR.try_into().unwrap(),
            );

            // Store character for later use
            let character: Character = Character {
                texture_id: texture,
                size: (
                    glyph.bitmap.width.try_into().unwrap(),
                    glyph.bitmap.rows.try_into().unwrap(),
                ),
                bearing: (glyph.bitmap_left, glyph.bitmap_top),
                advance: glyph.advance.x,
            };

            characters.insert(char::from(c as u8), character);
        }
        ft::FT_Done_Face(face);
        ft::FT_Done_Library(lib);
    };
}

fn init_opengl() {
    gl_loader::init_gl();
    gl::load_with(|symbol| gl_loader::get_proc_address(symbol) as *const _);
}

fn framebuffer_size_callback(_window: &mut glfw::Window, width: i32, height: i32) {
    println!(
        "Framebuffer size callback called with args {:?} {:?}",
        width, height
    );
    unsafe {
        gl::Viewport(0, 0, width.into(), height.into());
    }
}

fn load_object_into_mem(
    vertices: [f32; (NUM_VERTEX_ATTRIBS_RECT - 6) as usize],
    indices: [u32; NUM_INDICES_RECT as usize],
) -> (u32, u32) {
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
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (std::mem::size_of::<f32>() * vertices.len())
                .try_into()
                .unwrap(),
            vertices.as_ptr() as *const c_void,
            gl::STATIC_DRAW,
        );

        // generate and bind ebo
        gl::GenBuffers(1, &mut ebo);
        gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ebo);

        // Copy indices data into ebo
        gl::BufferData(
            gl::ELEMENT_ARRAY_BUFFER,
            (std::mem::size_of::<u32>() * indices.len())
                .try_into()
                .unwrap(),
            indices.as_ptr() as *const c_void,
            gl::STATIC_DRAW,
        );

        // Configure vertex attributes
        // position attrib
        gl::VertexAttribPointer(
            0,
            3,
            gl::FLOAT,
            gl::FALSE,
            (6 * std::mem::size_of::<f32>()).try_into().unwrap(),
            std::ptr::null(),
        );
        gl::EnableVertexAttribArray(0);
        // Color attrib
        gl::VertexAttribPointer(
            1,
            3,
            gl::FLOAT,
            gl::FALSE,
            (6 * std::mem::size_of::<f32>()).try_into().unwrap(),
            (3 * std::mem::size_of::<f32>()) as *const c_void,
        );
        gl::EnableVertexAttribArray(1);

        gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        gl::BindVertexArray(0);
    }

    (vao, ebo)
}

unsafe fn draw_object_from_mem(vao: u32, ebo: u32) {
    gl::BindVertexArray(vao);
    gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ebo);
    gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_INT, std::ptr::null());
    gl::BindVertexArray(0);
}

fn main() {
    let mut glfw = glfw::init_no_callbacks().unwrap();
    let (mut window, events) = glfw
        .create_window(
            WINDOW_WIDTH.into(),
            WINDOW_HEIGHT.into(),
            "rush",
            glfw::WindowMode::Windowed,
        )
        .expect("Failed to create window.");

    glfw::Window::set_framebuffer_size_callback(&mut window, framebuffer_size_callback);

    // Make the window's context current
    window.make_current();
    window.set_key_polling(true);

    init_opengl();

    unsafe {
        gl::Viewport(0, 0, WINDOW_WIDTH as i32, WINDOW_HEIGHT as i32);
    }

    let rect_one_vertex_attribs: [f32; (NUM_VERTEX_ATTRIBS_RECT - 6) as usize] = [
        // positions     // colors
        -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, // bottom left
        0.0, 0.0, 0.0, 0.0, 1.0, 0.0, // bottom right
        -1.0, 1.0, 0.0, 0.0, 0.0,
        1.0, // top left
             // 0.0,  1.0, 0.0, 0.0, 1.0, 0.0, // top right
    ];

    let rect_two_vertex_attribs: [f32; (NUM_VERTEX_ATTRIBS_RECT - 6) as usize] = [
        // Positions    // Colors
        0.0, -1.0, 0.0, 0.0, 0.0, 1.0, // bottom left
        1.0, -1.0, 0.0, 0.0, 1.0, 0.0, // bottom right
        0.0, 0.0, 0.0, 0.0, 1.0,
        0.0, // top left
             // 1.0,  0.0, 0.0, 1.0, 0.0, 0.0 // top right
    ];

    let rect_one_indices: [u32; 6] = [0, 1, 2, 1, 2, 3];

    let rect_two_indices: [u32; 6] = [0, 1, 2, 1, 2, 3];

    let (vao1, ebo1) = load_object_into_mem(rect_one_vertex_attribs, rect_one_indices);
    let (vao2, ebo2) = load_object_into_mem(rect_two_vertex_attribs, rect_two_indices);

    let dir = env::current_dir().expect("Could not get current directory");

    let vertex_path = dir.join("shader.vs");
    let fragment_path = dir.join("shader.fs");

    let shader = Shader::new(
        vertex_path.to_str().unwrap(),
        fragment_path.to_str().unwrap(),
    );

    // Loop until the user closes the window
    while !window.should_close() {
        window.swap_buffers();

        glfw.poll_events();
        for (_, event) in glfw::flush_messages(&events) {
            println!("{:?}", event);
            match event {
                glfw::WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                    window.set_should_close(true);
                }
                _ => {}
            }
        }

        unsafe {
            gl::ClearColor(0.2, 0.3, 0.3, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            shader.use_shader();
            let dt = glfw.get_time();
            println!("{:?}", dt.sin() / 2.0 + 0.5);
            let red: f64 = dt.sin() / 2.0 + 0.5;
            let green: f64 = dt.sin() / 2.0 + 0.5;
            let blue: f64 = dt.sin() / 2.0 + 0.5;
            let color_uniform_name = std::ffi::CString::new("color").unwrap();
            let vertex_color_location =
                gl::GetUniformLocation(*shader.get_id(), color_uniform_name.as_ptr());
            gl::Uniform4f(
                vertex_color_location,
                red as f32,
                green as f32,
                blue as f32,
                1.0,
            );

            draw_object_from_mem(vao1, ebo1);
            draw_object_from_mem(vao2, ebo2);
        }
    }
}
