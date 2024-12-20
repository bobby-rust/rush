#![allow(dead_code)]

mod shader;
mod yaml_parser;

extern crate freetype;
extern crate gl;
extern crate gl_loader;
extern crate glfw;
extern crate nalgebra_glm;

use freetype::freetype as ft;
use shader::Shader;
use glfw::Context;
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

impl std::fmt::Display for Grid {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Grid {{ rows: {}, cols: {}, cell_width: {}, cell_height: {} }}", self.rows, self.cols, self.cell_width, self.cell_height)
    }
}

struct WindowState {
    width: f32,
    height: f32,
    grid: Grid,
    // Keep one big buffer of the entire screen contents
    // Cells for each character need not be kept in memory
    // They can be derived from their location in the string
    buffer: String,
    // The index at which to begin rendering the buffer,
    // if the buffer is larger than the number of cells,
    // the first n buffer elements should not be rendered,
    // where n is the difference between the buffer size and
    // the size of the grid
    // For example,
    // if we have a 10x10 grid, that allows 100 characters.
    // if our buffer has 110 characters, only the last 100 characters
    // should be rendered. So n here is 10, 110 - 100
    display_offset: usize,
    next_cell: (usize, usize),
}

impl WindowState {
    fn new(width: f32, height: f32, char_dimensions: CharacterDimensions) -> WindowState {
        let cell_width = char_dimensions.width as f32;
        let cell_height = char_dimensions.height as f32;
        WindowState {
            width,
            height,
            grid: Grid {
                cell_width,
                cell_height,
                rows: height as usize / cell_height as usize,
                cols: width as usize / cell_width as usize,
            },
            buffer: String::new(),
            display_offset: 0,
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

    fn scroll(&mut self) {
        // just make the buffer begin rendering at 
        // ncols * rows_scrolled
        // So if we scroll down 2 rows,
        // the buffer should begin rendering at buffer[2 * ncols]
        // idk how to explain why this works with words but it works in my head
        // so thats good enough, it's because opengl doesn't have a concept of scrolling,
        // we have to replicate scrolling in terms of what the screen contents should be
        // after we scroll n rows, if we scroll 1 row, the last row of the screen should be blank,
        // and the top row of the screen should disappear.
        self.display_offset += self.grid.cols;
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
    events: glfw::GlfwReceiver<(f64, glfw::WindowEvent)>,
    glfw: glfw::Glfw,
    cursor_pos: (usize, usize), // Note that cursor_pos is always the location
}

struct Renderer {
    font_size_px: u32,
    font_shader: Shader,
    font_characters: Rc<RefCell<HashMap<char, Character>>>,
    font_vao: u32,
    font_vbo: u32,
    cursor_shader: Shader,
    cursor_vao: u32,
    cursor_vbo: u32,
    ebo: u32,
}

struct CharacterDimensions {
    width: u32,
    height: u32
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

fn load_font_chars(lib: ft::FT_Library, face: ft::FT_Face, font_size_px: u32) -> (HashMap<char, Character>, i64, i64) {
    let mut characters = HashMap::new();
    let mut max_advance = 0; // used to calculate the width of cells
    let mut max_height = 0;
    unsafe {
        ft::FT_Set_Pixel_Sizes(face, 0, font_size_px);

        gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);

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
                max_advance = glyph.advance.x >> 6;
            }


            gl::GenTextures(1, &mut texture);
            gl::BindTexture(gl::TEXTURE_2D, texture);
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0, gl::RED.try_into().unwrap(),
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

    (characters, max_advance, max_height)
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
    let mut ws = ws.borrow_mut();
    ws.reset_cell();
    renderer.font_shader.use_shader();

    unsafe {
        // Enable blending
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);

        let characters = renderer.font_characters.borrow();
        let buf = ws.buffer.clone();

        if buf[ws.display_offset..].len() + 1 > ws.grid.rows * ws.grid.cols {
            ws.scroll();
        }
        
        for c in buf[ws.display_offset..].chars() {
            let ftchar = characters.get(&c).unwrap();
            
            let (vertices, indices) = calculate_textured_quad_vertices(
                ws.get_next_cell(),
                ftchar,
                800.0,
                600.0,
                ws.grid.rows,
                ws.grid.cols
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
            ws.advance();
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
        // println!("No GL errors");
    }
}

fn key_to_capital_char(key: glfw::Key) -> Option<char> {
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
        _ => None,
    }
}
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

fn key_to_symbol(key: glfw::Key) -> Option<char> {
    match key {
        glfw::Key::Num1 => Some('1'),
        glfw::Key::Num2 => Some('2'),
        glfw::Key::Num3 => Some('3'),
        glfw::Key::Num4 => Some('4'),
        glfw::Key::Num5 => Some('5'),
        glfw::Key::Num6 => Some('6'),
        glfw::Key::Num7 => Some('7'),
        glfw::Key::Num8 => Some('8'),
        glfw::Key::Num9 => Some('9'),
        glfw::Key::Num0 => Some('0'),
        glfw::Key::Semicolon => Some(';'),
        glfw::Key::Comma => Some(','),
        glfw::Key::Period => Some('.'),
        glfw::Key::Slash => Some('/'),
        glfw::Key::Minus => Some('-'),
        glfw::Key::Equal => Some('='),
        glfw::Key::LeftBracket => Some('['),
        glfw::Key::RightBracket => Some(']'),
        glfw::Key::Backslash => Some('\\'),
        glfw::Key::GraveAccent => Some('`'),
        glfw::Key::Apostrophe => Some('\''),
        glfw::Key::Tab => Some('\t'),
        glfw::Key::Enter => Some('\n'),
        glfw::Key::Space => Some(' '),
        glfw::Key::Backspace => Some('_'),
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
    nrows: usize,
    ncols: usize,
    cell: (usize, usize),
) -> ([f32; 12], [u32; 6]) {
    let (row, col) = cell;

    // Calculate cell size in normalized coordinates
    let cell_width = 2.0 / ncols as f32;
    let cell_height = 2.0 / nrows as f32;

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
    nrows: usize,
    ncols: usize
) -> ([f32; 20], [u32; 6]) {
    let (row, col) = cell;

    // Cell dimensions
    let cell_width = 2.0 / ncols as f32;
    let cell_height = 2.0 / nrows as f32;

    // Top-left corner of the cell
    let cell_x = -1.0 + col as f32 * cell_width;
    let cell_y = 1.0 - (row as f32 + 1.0) * cell_height;

    let normalized_advance = (character.advance >> 6) as f32 / (window_width * 2.0);

    let usable_cell_width = cell_width - normalized_advance;

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

    let baseline_offset = character.bearing.1 as f32 / window_height * 2.0;

    // Center the character within the cell
    let char_x = cell_x + (cell_width - char_width) / 2.0;
    // Add 20% of the cell's height to the character's ypos,
    // maybe not the perfect solution but it works for now
    // Without the 20%, the baseline is rendered at the bottom of the cell,
    // so glyphs that go under the baseline overflow the cell
    let char_y = cell_y + baseline_offset - char_height + (cell_height * 0.2);


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
    glfw::GlfwReceiver<(f64, glfw::WindowEvent)>,
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
    unsafe { 
        glfw::ffi::glfwSetInputMode(glfw::Window::window_ptr(&window), glfw::ffi::LOCK_KEY_MODS, glfw::ffi::TRUE);
    };
    
    
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
    font_size_px: u32
) -> (
    freetype::freetype::FT_Library,
    freetype::freetype::FT_Face,
    Rc<RefCell<HashMap<char, Character>>>,
    CharacterDimensions
) {
    let lib = init_freetype_lib();
    let c_font_path = CString::new(font_path).unwrap();
    let face = create_ft_face(lib, &c_font_path);
    let (chars, max_width, max_height)= load_font_chars(lib, face, font_size_px);
    let char_dim = CharacterDimensions {
        width: max_width as u32, height: max_height as u32
    };

    (lib, face, Rc::new(RefCell::new(chars)), char_dim)
}

#[allow(unused)]
fn init() -> AppState {
    let config = yaml_parser::parse_config();
    let font_size = config.get("font_size").expect("Font size not found in config");
    let font_size_px: u32 = font_size.parse().expect("Invalid font size");
    let font_path = config.get("font_path").expect("Font path not found in config");
    let dir = env::current_dir().expect("Could not get current directory");
    let (glfw, mut window, events) = init_glfw_opengl(800.0, 600.0);
    let (font_shader, cursor_shader) = init_shaders(&dir);
    let (lib, face, characters, char_dim) =
        init_freetype(font_path, font_size_px);
    let (font_vao, font_vbo) = unsafe { make_text_vao_vbo() };
    let (cursor_vao, cursor_vbo, ebo) = make_cursor_vao_vbo_ebo();

    // Set up window callbacks
    window.borrow_mut().set_framebuffer_size_callback({
        let font_shader = font_shader.clone();
        move |_window, width, height| unsafe {
            gl::Viewport(0, 0, width.into(), height.into());
        }
    });

    let mut ws = Rc::new(RefCell::new(WindowState::new(800.0, 600.0, char_dim)));
    let app = AppState {
        ts: TerminalState {
            cursor_pos: (0, 0),
            glfw,
            events,
            window: window.to_owned(),
        },
        ws,
        renderer: Renderer {
            font_size_px,
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

    println!("{}", app.ws.borrow().grid);

    // window.borrow_mut().set_key_callback({
    //     // let chars = characters.clone();
    //     move |_window, key, _scancode, action, _modifiers| {
    //         if let Some(key_pressed) = key_to_char(key) {
    //             if action == glfw::Action::Press {
    //                 // let ch: &Character = characters.as_ref().borrow().get(&key_pressed).unwrap();
    //                 // let scale = 1.0;
    //
    //                 ws.borrow().buffer.push(key_pressed);
    //             }
    //         }
    //     }
    // });

    app
}

fn tick(app: &mut AppState) {
    app.ts.window.borrow_mut().swap_buffers();

    app.ts.glfw.poll_events();

    for (_, event) in glfw::flush_messages(&app.ts.events) {
        match event {
            glfw::WindowEvent::Key(glfw::Key::Escape, _, glfw::Action::Press, _) => {
                app.ts.window.borrow_mut().set_should_close(true);
            }

            glfw::WindowEvent::Key(key, _, glfw::Action::Press | glfw::Action::Repeat, modifiers) => {
                let mut ws = app.ws.borrow_mut();
                let ch; 
                if modifiers.contains(glfw::Modifiers::Shift) && modifiers.contains(glfw::Modifiers::CapsLock) {
                    if key > glfw::Key::Z || key < glfw::Key::A { 
                        ch = key_to_symbol(key); 
                    } else {
                        ch = key_to_char(key); 
                    }
                } else if modifiers.contains(glfw::Modifiers::Shift) || modifiers.contains(glfw::Modifiers::CapsLock) {
                    if key > glfw::Key::Z || key < glfw::Key::A { 
                        ch = key_to_symbol(key); 
                    } else {
                        ch = key_to_capital_char(key);
                    }
                } else {
                    if key > glfw::Key::Z || key < glfw::Key::A { 
                        ch = key_to_symbol(key); 
                    } else {
                        ch = key_to_char(key);
                    }
                }
                
                if ch == None { 
                    println!("Unrecognized key: {:?}", key);
                    return 
                };

                let c = ch.unwrap();

                match key {
                    glfw::Key::Backspace => {
                        ws.buffer.pop();
                    }
                    _ => {
                        ws.buffer.push(c);
                    }
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
            app.ws.borrow().grid.rows,
            app.ws.borrow().grid.cols,
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
