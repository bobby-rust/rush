#![allow(dead_code)]

mod shader;

extern crate freetype;
extern crate gl;
extern crate gl_loader;
extern crate glfw;
extern crate nalgebra_glm;

use freetype::freetype as ft;
use glfw::{Action, Context, Key, WindowEvent};
use shader::Shader;
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::ffi::CString;
use std::os::raw::c_void;
use std::rc::Rc;

struct Character {
    texture_id: u32,
    size: (i32, i32),
    bearing: (i32, i32),
    advance: i64,
}

struct Grid {
    rows: usize,
    cols: usize,
    cell_width: f32,
    cell_height: f32,
}

struct WindowState {
    width: f32,
    height: f32,
    grid: Grid,
    // Keep one big buffer of the entire screen contents
    // Cells for each character need not be kept in memory
    // They can be derived from their location in the string
    buffer: String,
    next_cell: (usize, usize),
}

impl WindowState {
    fn new(width: f32, height: f32) -> WindowState {
        WindowState {
            width,
            height,
            grid: Grid {
                rows: 24,
                cols: 80,
                cell_width: width / 80.0,
                cell_height: height / 24.0,
            },
            buffer: String::new(),
            next_cell: (0, 0),
        }
    }

    fn advance(&mut self) {
        if self.next_cell.1 == self.grid.cols - 1 {
            self.next_cell = (self.next_cell.0 + 1, 0);
        } else {
            self.next_cell = (self.next_cell.0, self.next_cell.1 + 1);
        }
    }

    fn reset_cell(&mut self) {
        self.next_cell = (0, 0);
    }

    fn update_size(&mut self, width: f32, height: f32) {
        self.width = width;
        self.height = height;
        self.grid.rows = (self.height / self.grid.cell_height) as usize;
        self.grid.cols = (self.width / self.grid.cell_width) as usize;
    }

    fn get_next_cell(&self) -> (usize, usize) {
        self.next_cell
    }
}

struct AppState {
    ts: TerminalState,
    ws: Rc<RefCell<WindowState>>,
    renderer: Renderer,
}

struct TerminalState {
    window: Rc<RefCell<glfw::PWindow>>,
    events: glfw::GlfwReceiver<(f64, WindowEvent)>,
    glfw: glfw::Glfw,
    cursor_pos: (usize, usize), // Note that cursor_pos is always the location
}

struct Renderer {
    font_shader: Shader,
    font_characters: Rc<RefCell<HashMap<char, Character>>>,
    font_vao: u32,
    font_vbo: u32,
    cursor_shader: Shader,
    cursor_vao: u32,
    cursor_vbo: u32,
    ebo: u32,
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

fn load_font_chars(lib: ft::FT_Library, face: ft::FT_Face) -> HashMap<char, Character> {
    let mut characters = HashMap::new();
    unsafe {
        ft::FT_Set_Pixel_Sizes(face, 0, 12);

        gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);

        let mut max_advance = 0; // advance is used as width
        let mut max_height = 0;
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
            if glyph.advance.x > max_advance {
                max_advance = glyph.advance.x;
            }

            println!("Advance for {}: {}", c, glyph.metrics.horiAdvance);

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
        gl::BindTexture(gl::TEXTURE_2D, 0);

        ft::FT_Done_Face(face);
        ft::FT_Done_Library(lib);
    };

    characters
}

unsafe fn make_text_vao_vbo() -> (u32, u32) {
    let mut vao: u32 = 0;
    let mut vbo: u32 = 0;

    // Create and bind VAO
    gl::GenVertexArrays(1, &mut vao);
    gl::BindVertexArray(vao);

    // Create and bind VBO
    gl::GenBuffers(1, &mut vbo);
    gl::BindBuffer(gl::ARRAY_BUFFER, vbo);

    // Fill VBO with geometry data
    gl::BufferData(
        gl::ARRAY_BUFFER,
        (std::mem::size_of::<f32>() * 4 * 5) as isize,
        std::ptr::null(),
        gl::STATIC_DRAW,
    );

    // Set the position attribute (3 floats per vertex for position)
    gl::VertexAttribPointer(
        0,
        3,
        gl::FLOAT,
        gl::FALSE,
        5 * std::mem::size_of::<f32>() as i32,
        // Byte offset. The position comes first at the beginning of the array, thus null for no
        // offset
        std::ptr::null(),
    );
    gl::EnableVertexAttribArray(0);

    // Set texture coordinates attribute
    gl::VertexAttribPointer(
        1,
        2,
        gl::FLOAT,
        gl::FALSE,
        5 * std::mem::size_of::<f32>() as i32,
        // Byte offset to first element. We have 5 floats, first 3 x, y, z, last 2 2d texture
        // coords x, y. Texture coords start at index 3.
        (3 * std::mem::size_of::<f32>()) as *const _, // byte offset to first element
    );
    gl::EnableVertexAttribArray(1);

    (vao, vbo)
}

fn make_cursor_vao_vbo_ebo() -> (u32, u32, u32) {
    let mut vao: u32 = 0;
    let mut vbo: u32 = 0;
    let mut ebo: u32 = 0;

    let vertices: [f32; 12] = [
        -0.5, 0.5, 0.0, // top left
        0.5, 0.5, 0.0, // top right
        -0.5, -0.5, 0.0, // bottom left
        0.5, -0.5, 0.0, // bottom right
    ];

    let indices: [u32; 6] = [0, 1, 2, 1, 2, 3];

    unsafe {
        // Create and bind VAO
        gl::GenVertexArrays(1, &mut vao);
        gl::BindVertexArray(vao);

        // Create and bind VBO
        gl::GenBuffers(1, &mut vbo);
        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);

        // Fill VBO with geometry data
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (std::mem::size_of::<f32>() * vertices.len()) as isize,
            vertices.as_ptr() as *const c_void,
            gl::STATIC_DRAW,
        );

        gl::VertexAttribPointer(
            0,
            3,
            gl::FLOAT,
            gl::FALSE,
            3 * std::mem::size_of::<f32>() as i32,
            std::ptr::null(),
        );
        gl::EnableVertexAttribArray(0);

        // Create and bind EBO
        gl::GenBuffers(1, &mut ebo);
        gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ebo);

        // Fill EBO with indices data
        gl::BufferData(
            gl::ELEMENT_ARRAY_BUFFER,
            (std::mem::size_of::<u32>() * indices.len()) as isize,
            indices.as_ptr() as *const c_void,
            gl::STATIC_DRAW,
        );

        // gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        // gl::BindVertexArray(0);
    }

    (vao, vbo, ebo)
}

fn render_screen_buffer(renderer: &Renderer, ws: Rc<RefCell<WindowState>>) {
    println!("Rendering buffer: {}", ws.borrow().buffer);
    ws.borrow_mut().reset_cell();
    renderer.font_shader.use_shader();
    // let program = renderer.font_shader.get_id();
    unsafe {
        // Enable blending
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);

        // let text_location = gl::GetUniformLocation(*program, b"text".as_ptr() as *const i8);
        // gl::Uniform1i(text_location, 0);

        let characters = renderer.font_characters.borrow();
        let buf = ws.borrow().buffer.clone();
        for c in buf.chars() {
            let ftchar = characters.get(&c).unwrap();

            let (vertices, indices) = calculate_textured_quad_vertices(
                ws.borrow_mut().get_next_cell(),
                ftchar,
                800.0,
                600.0,
            );
            set_renderer_vertices(renderer.font_vao, renderer.font_vbo, &vertices, &indices);

            // Set the active texture
            gl::ActiveTexture(gl::TEXTURE0);

            // Bind the VAO
            gl::BindVertexArray(renderer.font_vao);

            // Bind texture
            gl::BindTexture(gl::TEXTURE_2D, ftchar.texture_id);

            // Bind the buffer
            gl::BindBuffer(gl::ARRAY_BUFFER, renderer.font_vbo);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, renderer.ebo);

            // check_gl_errors();

            gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_INT, std::ptr::null());
            ws.borrow_mut().advance();
        }
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
    } else {
        println!("No GL errors");
    }
}

#[allow(unused)]
fn key_to_char(key: glfw::Key) -> Option<char> {
    match key {
        glfw::Key::A => Some('a'),
        glfw::Key::B => Some('b'),
        glfw::Key::C => Some('c'),
        glfw::Key::D => Some('d'),
        glfw::Key::E => Some('e'),
        glfw::Key::F => Some('f'),
        glfw::Key::G => Some('g'),
        glfw::Key::H => Some('h'),
        glfw::Key::I => Some('i'),
        glfw::Key::J => Some('j'),
        glfw::Key::K => Some('k'),
        glfw::Key::L => Some('l'),
        glfw::Key::M => Some('m'),
        glfw::Key::N => Some('n'),
        glfw::Key::O => Some('o'),
        glfw::Key::P => Some('p'),
        glfw::Key::Q => Some('q'),
        glfw::Key::R => Some('r'),
        glfw::Key::S => Some('s'),
        glfw::Key::T => Some('t'),
        glfw::Key::U => Some('u'),
        glfw::Key::V => Some('v'),
        glfw::Key::W => Some('w'),
        glfw::Key::X => Some('x'),
        glfw::Key::Y => Some('y'),
        glfw::Key::Z => Some('z'),
        _ => None,
    }
}

#[allow(unused)]
fn translation_matrix(dx: f32, dy: f32, width: f32, height: f32) -> [[f32; 4]; 4] {
    let ndc_dx = dx / width * 2.0;
    let ndc_dy = dy / height * 2.0;

    [
        [1.0, 0.0, 0.0, ndc_dx],
        [0.0, 1.0, 0.0, ndc_dy],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn set_uniform_mat4(s: &Shader, uniform_name: std::ffi::CString, transform: [[f32; 4]; 4]) {
    let location = unsafe { gl::GetUniformLocation(*s.get_id(), uniform_name.as_ptr()) };
    unsafe {
        gl::UniformMatrix4fv(location, 1, gl::FALSE, transform.as_ptr() as *const f32);
    }
}

fn render_cursor(s: &Shader, vao: u32) {
    s.use_shader();
    unsafe {
        gl::Disable(gl::CULL_FACE);
    };

    unsafe {
        gl::BindVertexArray(vao);
        gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_INT, std::ptr::null());
        gl::BindVertexArray(0);
    }
}

fn calculate_cursor_vertices(
    _window_width: f32,
    _window_height: f32,
    cell: (usize, usize),
) -> ([f32; 12], [u32; 6]) {
    let (row, col) = cell;

    // Calculate cell size in normalized coordinates
    let cell_width = 2.0 / 80.0;
    let cell_height = 2.0 / 24.0;

    // Calculate bottom-left corner in normalized coordinates
    let x = -1.0 + col as f32 * cell_width;
    let y = 1.0 - (row as f32 + 1.0) * cell_height;

    // Create the vertex positions for the cell
    let vertices = [
        x,
        y + cell_height,
        0.0, // Top left
        x + cell_width,
        y + cell_height,
        0.0, // Top right
        x,
        y,
        0.0, // Bottom left
        x + cell_width,
        y,
        0.0, // Bottom right
    ];

    // Define the indices for two triangles forming a rectangle
    let indices = [
        0, 1, 2, // First triangle
        1, 2, 3, // Second triangle
    ];

    (vertices, indices)
}

fn calculate_textured_quad_vertices(
    cell: (usize, usize),
    character: &Character,
    window_width: f32,
    window_height: f32,
) -> ([f32; 20], [u32; 6]) {
    let (row, col) = cell;

    // Cell dimensions
    let cell_width = 2.0 / 80.0;
    let cell_height = 2.0 / 24.0;

    // Top-left corner of the cell
    let cell_x = -1.0 + col as f32 * cell_width;
    let cell_y = 1.0 - (row as f32 + 1.0) * cell_height;

    let normalized_advance = (character.advance >> 6) as f32 / (window_width * 2.0);

    let usable_cell_width = cell_width - normalized_advance;

    println!(
        "Usable cell width: {}, normalized advance: {}, cell_width: {}",
        usable_cell_width, normalized_advance, cell_width
    );
    // Character dimensions
    let mut char_width = character.size.0 as f32 / window_width * 2.0;
    let mut char_height = character.size.1 as f32 / window_height * 2.0;

    if char_width > usable_cell_width {
        char_width = usable_cell_width;
    }
    if char_height > cell_height {
        // let scale = cell_height / char_height;
        // char_width *= scale;
        char_height = cell_height;
    }
    // Center the character within the cell
    let char_x = cell_x + (cell_width - char_width) / 2.0;
    let char_y = cell_y + (cell_height - char_height) / 2.0;

    // println!(
    //     "char_x: {}, char_y: {}, char_width: {}, char_height: {}, cell_width: {}, cell_height: {}",
    //     char_x, char_y, char_width, char_height, cell_width, cell_height
    // );

    let vertices = [
        char_x,
        char_y + char_height,
        0.0,
        0.0,
        0.0,
        char_x + char_width,
        char_y + char_height,
        0.0,
        1.0,
        0.0,
        char_x,
        char_y,
        0.0,
        0.0,
        1.0,
        char_x + char_width,
        char_y,
        0.0,
        1.0,
        1.0,
    ];

    let indices = [
        0, 1, 2, // First triangle
        1, 2, 3, // Second triangle
    ];

    (vertices, indices)
}

fn set_renderer_vertices(vao: u32, vbo: u32, vertices: &[f32], _indices: &[u32]) {
    unsafe {
        gl::BindVertexArray(vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (std::mem::size_of::<f32>() * vertices.len()) as isize,
            vertices.as_ptr() as *const c_void,
            gl::STATIC_DRAW,
        );

        // Unbind
        gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        gl::BindVertexArray(0);
    }
}

fn init_glfw(
    window_width: f32,
    window_height: f32,
) -> (
    glfw::Glfw,
    glfw::PWindow,
    glfw::GlfwReceiver<(f64, WindowEvent)>,
) {
    let mut glfw = glfw::init_no_callbacks().unwrap();
    let (mut window, events) = glfw
        .create_window(
            window_width as u32,
            window_height as u32,
            "rush",
            glfw::WindowMode::Windowed,
        )
        .expect("Failed to create window.");

    // Make the window's context current
    window.make_current();
    window.set_key_polling(true);

    (glfw, window, events)
}

fn init_glfw_opengl(
    window_width: f32,
    window_height: f32,
) -> (
    glfw::Glfw,
    Rc<RefCell<glfw::PWindow>>,
    glfw::GlfwReceiver<(f64, glfw::WindowEvent)>,
) {
    let (glfw, window, events) = init_glfw(window_width, window_height);
    init_opengl();
    unsafe {
        gl::Viewport(0, 0, window_width as i32, window_height as i32);
    }
    (glfw, Rc::new(RefCell::new(window)), events)
}

fn init_shaders(dir: &std::path::Path) -> (Shader, Shader) {
    let font_shader = Shader::new(
        dir.join("font_shader.vs").to_str().unwrap(),
        dir.join("font_shader.fs").to_str().unwrap(),
    );

    let cursor_shader = Shader::new(
        dir.join("cursor_shader.vs").to_str().unwrap(),
        dir.join("cursor_shader.fs").to_str().unwrap(),
    );

    (font_shader, cursor_shader)
}

fn init_freetype(
    font_path: &str,
) -> (
    freetype::freetype::FT_Library,
    freetype::freetype::FT_Face,
    Rc<RefCell<HashMap<char, Character>>>,
) {
    let lib = init_freetype_lib();
    let c_font_path = CString::new(font_path).unwrap();
    let face = create_ft_face(lib, &c_font_path);
    unsafe { freetype::freetype::FT_Set_Pixel_Sizes(face, 0, 48) };
    let characters = load_font_chars(lib, face);
    (lib, face, Rc::new(RefCell::new(characters)))
}

#[allow(unused)]
fn init() -> AppState {
    let dir = env::current_dir().expect("Could not get current directory");
    let (glfw, mut window, events) = init_glfw_opengl(800.0, 600.0);
    let (font_shader, cursor_shader) = init_shaders(&dir);
    let (lib, face, characters) =
        init_freetype("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf");
    let (font_vao, font_vbo) = unsafe { make_text_vao_vbo() };
    let (cursor_vao, cursor_vbo, ebo) = make_cursor_vao_vbo_ebo();

    // Set up window callbacks
    window.borrow_mut().set_framebuffer_size_callback({
        let font_shader = font_shader.clone();
        move |_window, width, height| unsafe {
            gl::Viewport(0, 0, width.into(), height.into());
        }
    });

    let mut ws = WindowState::new(800.0, 600.0);
    let app = AppState {
        ts: TerminalState {
            cursor_pos: (0, 0),
            glfw,
            events,
            window: window.to_owned(),
        },
        ws: Rc::new(RefCell::new(WindowState::new(800.0, 600.0))),
        renderer: Renderer {
            font_vao,
            font_vbo,
            cursor_vao,
            cursor_vbo,
            font_shader,
            font_characters: characters.clone(),
            cursor_shader,
            ebo,
        },
    };

    window.borrow_mut().set_key_callback({
        // let chars = characters.clone();
        move |_window, key, _scancode, action, _modifiers| {
            if let Some(key_pressed) = key_to_char(key) {
                if action == glfw::Action::Press {
                    // let ch: &Character = characters.as_ref().borrow().get(&key_pressed).unwrap();
                    // let scale = 1.0;

                    ws.buffer.push(key_pressed);
                }
            }
        }
    });

    app
}

fn tick(app: &mut AppState) {
    app.ts.window.borrow_mut().swap_buffers();

    app.ts.glfw.poll_events();

    for (_, event) in glfw::flush_messages(&app.ts.events) {
        match event {
            glfw::WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                app.ts.window.borrow_mut().set_should_close(true);
            }
            glfw::WindowEvent::Key(key, _, Action::Press | Action::Repeat, _) => {
                if let Some(ch) = key_to_char(key) {
                    let mut ws = app.ws.borrow_mut();
                    ws.buffer.push(ch);
                }
            }
            _ => {}
        }
    }

    check_gl_errors();
    unsafe {
        //gl::PolygonMode(gl::FRONT_AND_BACK, gl::LINE);
        gl::ClearColor(0.0, 0.0, 0.0, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);

        render_screen_buffer(&app.renderer, app.ws.clone());

        let (cursor_vertices, cursor_indices) = calculate_cursor_vertices(
            app.ws.borrow().width,
            app.ws.borrow().height,
            app.ws.borrow().get_next_cell(),
        );

        set_renderer_vertices(
            app.renderer.cursor_vao,
            app.renderer.cursor_vbo,
            &cursor_vertices,
            &cursor_indices,
        );
        render_cursor(&app.renderer.cursor_shader, app.renderer.cursor_vbo);
    }
}

fn main() {
    let mut app: AppState = init();
    check_gl_errors();
    while !app.ts.window.as_ref().borrow().should_close() {
        tick(&mut app);
    }
}
