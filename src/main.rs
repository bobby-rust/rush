mod shader;

extern crate freetype;
extern crate gl;
extern crate gl_loader;
extern crate glfw;
extern crate nalgebra_glm;

use freetype::freetype as ft;
use glfw::{Action, Context, Key};
use nalgebra_glm as glm;
use shader::Shader;
use std::collections::HashMap;
use std::env;
use std::os::raw::c_void;
use std::cell::RefCell;
use std::rc::Rc;

struct Character {
    texture_id: u32,
    size: (i32, i32),
    bearing: (i32, i32),
    advance: i64,
}

struct Cell {
    width: f32,
    height: f32,
}

struct Grid {
    rows: u32,
    cols: u32,
}

fn init_freetype_lib() -> ft::FT_Library {
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
    let mut face: ft::FT_Face = std::ptr::null_mut();
    let error = unsafe { ft::FT_New_Face(lib, font_path.as_ptr(), 0, &mut face) };
    if error != 0 {
        panic!("Could not create font face. ERROR CODE: {:?}", error);
    }

    face
}

fn load_font_chars(lib: ft::FT_Library, face: ft::FT_Face) -> Rc<RefCell<HashMap<char, Character>>> {
    let characters = Rc::new(RefCell::new(HashMap::new()));
    unsafe {
        ft::FT_Set_Pixel_Sizes(face, 0, 48);

        gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
        
        let mut max_advance = 0; // advance is used as width
        let mut max_height  = 0;
        for c in 0..127 {
            let error = ft::FT_Load_Char(face, c, ft::FT_LOAD_RENDER as i32);
            if error != 0 {
                panic!("Could not load character. ERROR CODE: {:?}", error);
            }

            // Generate texture
            let mut texture: u32 = 0;
            let glyph = &*(*face).glyph;
            let metrics = (*(*face).size).metrics;
            if (metrics.height >> 6) > max_height {
                max_height = metrics.height >> 6;
            }
            if glyph.advance.x > max_advance { max_advance = glyph.advance.x; }

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

            characters.borrow_mut().insert(char::from(c as u8), character);
        }
        gl::BindTexture(gl::TEXTURE_2D, 0);

        ft::FT_Done_Face(face);
        ft::FT_Done_Library(lib);
    };

    characters
}

unsafe fn make_text_vao_vbo() -> (u32, u32) {
    gl::Enable(gl::BLEND);
    gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);

    let mut vao: u32 = 0;
    let mut vbo: u32 = 0;

    gl::GenVertexArrays(1, &mut vao);
    gl::GenBuffers(1, &mut vbo);
    gl::BindVertexArray(vao);
    gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
    gl::BufferData(
        gl::ARRAY_BUFFER,
        (std::mem::size_of::<f32>() * 6 * 4) as isize,
        std::ptr::null(),
        gl::DYNAMIC_DRAW,
    );

    gl::EnableVertexAttribArray(0);
    gl::VertexAttribPointer(
        0,
        4,
        gl::FLOAT,
        gl::FALSE,
        4 * std::mem::size_of::<f32>() as i32,
        std::ptr::null(),
    );
    gl::BindBuffer(gl::ARRAY_BUFFER, 0);
    gl::BindVertexArray(0);

    (vao, vbo)
}

fn render_text(
    text: String,
    window_width: f32,
    window_height: f32,
    // x: f32,
    // y: f32,
    scale: f32,
    color: glm::Vec3,
    s: &Shader,
    vao: u32,
    vbo: u32,
    characters: &HashMap<char, Character>,
) {
    s.use_shader();
    unsafe { gl::Enable(gl::CULL_FACE); };

    let mut x: f32 = 0.0;
    let mut nlines = 1;

    let uniform_color_var_name =
        std::ffi::CString::new("textColor").expect("Could not create C string.");

    unsafe {
        gl::Uniform3f(
            gl::GetUniformLocation(*s.get_id(), uniform_color_var_name.as_ptr()),
            color.x,
            color.y,
            color.z,
        );

        gl::ActiveTexture(gl::TEXTURE0);
        gl::BindVertexArray(vao);

        let mut current_row = 0;
        let mut current_column = 0;

        for c in text.chars() {
            let ch: &Character = characters.get(&c).unwrap();

            let w: f32 = ch.size.0 as f32 * scale;
            let h: f32 = ch.size.1 as f32 * scale;

            let num_columns = (window_width / w) as usize;
            let num_rows =    (window_width / h) as usize;

            if (x + w) > window_width {
                x = 0.0;
                nlines += 1;
            }

            let y = window_height - ((47.0 * scale) * nlines as f32) as f32; // 47 is the largest
            let xpos: f32 = x + ch.bearing.0 as f32 * scale;
            let ypos: f32 = y as f32 - ((ch.size.1 - ch.bearing.1) as f32 * scale);
            x += (ch.advance / 64) as f32 * scale;

            // update vbo for each character
            let vertices: [[f32; 4]; 6] = [
                [xpos, ypos + h, 0.0, 0.0],
                [xpos, ypos, 0.0, 1.0],
                [xpos + w, ypos, 1.0, 1.0],
           
               [xpos, ypos + h, 0.0, 0.0],
               [xpos + w, ypos, 1.0, 1.0],
               [xpos + w, ypos + h, 1.0, 0.0],
           ];


            // Render glyph texture over quad
            gl::BindTexture(gl::TEXTURE_2D, ch.texture_id);
            // update content of vbo memory
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            gl::BufferSubData(
                gl::ARRAY_BUFFER,
                0,
                std::mem::size_of::<[[f32; 4]; 6]>() as isize,
                vertices.as_ptr() as *const c_void,
            );
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
            // render quad
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
        }

        gl::BindVertexArray(0);
        gl::BindTexture(gl::TEXTURE_2D, 0);
    }
}

fn init_opengl() {
    gl_loader::init_gl();
    gl::load_with(|symbol| gl_loader::get_proc_address(symbol) as *const _);
}

fn check_gl_errors() {
    let err = unsafe { gl::GetError() };
    if err != gl::NO_ERROR {
        println!("GL error: {:?}", err);
    }
}

fn key_to_char(key: glfw::Key) -> Option<char> {
    match key {
        glfw::Key::A => Some('A'),
        glfw::Key::B => Some('B'),
        glfw::Key::C => Some('C'),
        glfw::Key::D => Some('D'),
        glfw::Key::E => Some('E'),
        glfw::Key::F => Some('F'),
        glfw::Key::G => Some('G'),
        glfw::Key::H => Some('H'),
        glfw::Key::I => Some('I'),
        glfw::Key::J => Some('J'),
        glfw::Key::K => Some('K'),
        glfw::Key::L => Some('L'),
        glfw::Key::M => Some('M'),
        glfw::Key::N => Some('N'),
        glfw::Key::O => Some('O'),
        glfw::Key::P => Some('P'),
        glfw::Key::Q => Some('Q'),
        glfw::Key::R => Some('R'),
        glfw::Key::S => Some('S'),
        glfw::Key::T => Some('T'),
        glfw::Key::U => Some('U'),
        glfw::Key::V => Some('V'),
        glfw::Key::W => Some('W'),
        glfw::Key::X => Some('X'),
        glfw::Key::Y => Some('Y'),
        glfw::Key::Z => Some('Z'),
        _ => None 
    }
}

fn translation_matrix(dx: f32, dy: f32, width: f32, height: f32) -> [[f32; 4]; 4] {
    let ndc_dx = dx / width * 2.0;
    let ndc_dy = dy / height * 2.0;
    [
        [1.0, 0.0, 0.0, ndc_dx],
        [0.0, 1.0, 0.0, ndc_dy],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0]
    ]
}

fn set_uniform_mat4(s: &Shader, uniform_name: std::ffi::CString, transform: [[f32; 4]; 4]) {
    let location = unsafe { gl::GetUniformLocation(*s.get_id(), uniform_name.as_ptr()) };
    unsafe { 
        gl::UniformMatrix4fv(location, 1, gl::FALSE, transform.as_ptr() as *const f32);
    }
}

fn render_cursor(s: &Shader, vao: u32, transform: [[f32; 4]; 4]) {
    s.use_shader();
    unsafe { gl::Disable(gl::CULL_FACE); };
    set_uniform_mat4(s, std::ffi::CString::new("transform").unwrap(), transform);

    unsafe {
        gl::BindVertexArray(vao);
        gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_INT, std::ptr::null());
        gl::BindVertexArray(0);
    }
}

fn calculate_translation_matrix(row: u32, col: u32, nrows: u32, ncols: u32, window_width: f32, window_height: f32) -> [[f32; 4]; 4] {
    let scale_x = 2.0 / window_width;
    let scale_y = 2.0 / window_height;

    let cell_width = window_width / ncols as f32;
    let cell_height = window_height / nrows as f32;

    let ndc_x = (col as f32 + 0.5) * cell_width * scale_x - 1.0;
    let ndc_y = 1.0 - (row as f32 + 0.5) * cell_height * scale_y;
    [
        [scale_x * cell_width, 0.0, 0.0, 0.0],
        [0.0, scale_y * cell_height, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [ndc_x, ndc_y, 0.0, 1.0]
    ]
}

fn make_cursor_vao_vbo_ebo() -> (u32, u32) {
    let mut vao: u32 = 0;
    let mut vbo: u32 = 0;
    let mut ebo: u32 = 0;

    let vertices: [f32; 12] = [
        -0.5, 0.5, 0.0,  // top left
        0.5, 0.5, 0.0,   // top right
        -0.5, -0.5, 0.0, // bottom left
        0.5, -0.5, 0.0,  // bottom right 
    ];

    let indices: [u32; 6] = [
        0, 1, 2,
        1, 2, 3
    ];

    unsafe { 
        gl::GenVertexArrays(1, &mut vao);
        gl::GenBuffers(1, &mut vbo);
        gl::GenBuffers(1, &mut ebo);
        gl::BindVertexArray(vao);

        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
        gl::BufferData(gl::ARRAY_BUFFER, (std::mem::size_of::<f32>() * vertices.len()) as isize, vertices.as_ptr() as *const c_void, gl::STATIC_DRAW);

        gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ebo);
        gl::BufferData(gl::ELEMENT_ARRAY_BUFFER, (std::mem::size_of::<f32>() * indices.len()) as isize, indices.as_ptr() as *const c_void, gl::STATIC_DRAW);

        gl::VertexAttribPointer(0, 3, gl::FLOAT, gl::FALSE, 3 * std::mem::size_of::<f32>() as i32, std::ptr::null());
        gl::EnableVertexAttribArray(0);

        gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        gl::BindVertexArray(0);
    }

    (vao, vbo)
}

fn main() {

    let grid = Grid {
        rows: 24,
        cols: 80
    };

    let cell = Cell {
        width: 800.0 / grid.cols as f32,
        height: 600.0 / grid.rows as f32
    };

    let window_width = Rc::new(RefCell::new(800.0));
    let window_height = Rc::new(RefCell::new(600.0));
    let mut glfw = glfw::init_no_callbacks().unwrap();
    let (mut window, events) = glfw
        .create_window(
            *window_width.borrow() as u32,
            *window_height.borrow() as u32,
            "rush",
            glfw::WindowMode::Windowed,
        )
        .expect("Failed to create window.");

    init_opengl();

    // Make the window's context current
    window.make_current();
    window.set_key_polling(true);

    unsafe {
        gl::Viewport(0, 0, *window_width.borrow() as i32, *window_height.borrow() as i32);
    }
    
    let dir = env::current_dir().expect("Could not get current directory");

    let text_vertex_path = dir.join("text_shader.vs");
    let text_fragment_path = dir.join("text_shader.fs");

    let text_shader = Rc::new(RefCell::new(
        Shader::new(text_vertex_path.to_str().unwrap(), text_fragment_path.to_str().unwrap())
    ));
    
    let cursor_vertex_path = dir.join("cursor_shader.vs");
    let cursor_fragment_path = dir.join("cursor_shader.fs");

    let cursor_shader = Shader::new(cursor_vertex_path.to_str().unwrap(), cursor_fragment_path.to_str().unwrap());

    let uniform_projection_var_name =
        std::ffi::CString::new("projection").expect("Could not create C string");
    let uniform_transform_var_name =
        std::ffi::CString::new("transform").expect("Could not create C string");

    
    text_shader.borrow().use_shader();
    set_uniform_mat4(&*text_shader.borrow(), uniform_projection_var_name.clone(), glm::ortho(0.0, 800.0, 0.0, 600.0, -1.0, 1.0).into());

    // let transform: [[f32; 4]; 4] = [
    //     [0.05, 0.0, 0.0, 0.0],
    //     [0.0, 0.1, 0.0, 0.0],
    //     [0.0, 0.0, 1.0, 0.0],
    //     [-0.9, 0.9, 0.0, 1.0],
    // ];
    //
    let mut transform = Rc::new(RefCell::new(calculate_translation_matrix(10, 10, grid.rows, grid.cols, 800.0, 600.0)));
    cursor_shader.use_shader();
    set_uniform_mat4(&cursor_shader, uniform_transform_var_name, *transform.borrow());

    glfw::Window::set_framebuffer_size_callback(&mut window, {
        let text_shader_clone = text_shader.clone();
        let window_width_clone = window_width.clone();
        let window_height_clone = window_height.clone();
        move |_window, width, height| {
            *window_width_clone.borrow_mut() = width as f32;
            *window_height_clone.borrow_mut() = height as f32;
            unsafe { 
                gl::Viewport(0, 0, width.into(), height.into());
                gl::UniformMatrix4fv(
                    gl::GetUniformLocation(*text_shader_clone.borrow().get_id(), uniform_projection_var_name.as_ptr()),
                    1,
                    gl::FALSE,
                    glm::ortho(0.0, width as f32, 0.0, height as f32, -1.0, 1.0).as_slice().as_ptr(),
                );
            } 
        }
    });

    check_gl_errors();

    let lib = init_freetype_lib();
    let font_path = std::ffi::CString::new("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf")
        .expect("Failed to create C string for font path");
    let face = create_ft_face(lib, &font_path);
    unsafe { 
        freetype::freetype::FT_Set_Pixel_Sizes(face, 0, 48);
    }

    let characters = load_font_chars(lib, face);

    let (text_vao, text_vbo) = unsafe { make_text_vao_vbo() };
    let (cursor_vao, cursor_vbo) = make_cursor_vao_vbo_ebo();

    // unsafe { gl::PolygonMode(gl::FRONT_AND_BACK, gl::LINE); }
    
    // Loop until the user closes the window
    let x = Rc::new(RefCell::new(0.0));
    let typed_text = Rc::new(RefCell::new(String::new()));

    window.set_key_callback({
        let typed_text_clone = Rc::clone(&typed_text);
        let characters_clone = Rc::clone(&characters);
        let x_clone = Rc::clone(&x);
        let transform_clone = Rc::clone(&transform);
        move |_window, key, _scancode, action, _modifiers| {
            let mut typed_text_borrow = typed_text_clone.borrow_mut();
            let characters_borrow = characters_clone.borrow_mut();
            let mut x_borrow = x_clone.borrow_mut();
            let mut transform_borrow = transform_clone.borrow_mut();
            if let Some(key_pressed) = key_to_char(key) {
                if  action == glfw::Action::Press  {
                    let ch: &Character = characters_borrow.get(&key_pressed).unwrap();
                    let scale = 1.0;
                    *x_borrow += (ch.advance >> 6) as f32 * scale;

                    typed_text_borrow.push(key_pressed);
                    *transform_borrow = calculate_translation_matrix(15, 15, grid.rows, grid.cols, 800.0, 600.0);
                }
            }
        }
    });

    // unsafe { gl::Enable(gl::DEPTH_TEST); };
    
    check_gl_errors();
    while !window.should_close() {
        window.swap_buffers();

        glfw.poll_events();
        for (_, event) in glfw::flush_messages(&events) {
            
            match event {
                glfw::WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                    window.set_should_close(true);
                }
                _ => {}
            }
        }

        unsafe {
            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            render_text(
                typed_text.borrow().to_string(),
                *window_width.borrow(),
                *window_height.borrow(),
                // *x.borrow_mut(),
                // WINDOW_HEIGHT as f32 - 40.0,
                0.5,
                glm::vec3(0.5, 0.8, 0.2),
                &text_shader.borrow(),
                text_vao,
                text_vbo,
                &characters.borrow_mut(),
            );
            render_cursor(&cursor_shader, cursor_vbo, *transform.borrow());
        }
    }
}

