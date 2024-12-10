#![allow(dead_code)]

mod shader;

extern crate freetype;
extern crate gl;
extern crate gl_loader;
extern crate glfw;
extern crate nalgebra_glm;

use freetype::freetype as ft;
use glfw::{Action, Context, Key, WindowEvent};
use nalgebra_glm as glm;
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

    fn update_size(&mut self, width: f32, height: f32) {
        self.width = width;
        self.height = height;
        self.grid.rows = (self.height / self.grid.cell_height) as usize;
        self.grid.cols = (self.width / self.grid.cell_width) as usize;
    }

    fn get_next_cell(&mut self) -> (usize, usize) {
        if self.next_cell.1 == self.grid.cols - 1 {
            self.next_cell = (self.next_cell.0 + 1, 0);
        } else {
            self.next_cell = (self.next_cell.0, self.next_cell.1 + 1);
        }

        self.next_cell
    }
}

struct AppState {
    ts: TerminalState,
    ws: WindowState,
    renderer: Renderer
}

struct TerminalState {
    buffer: String,
    window: Rc<RefCell<glfw::PWindow>>,
    events: glfw::GlfwReceiver<(f64, WindowEvent)>,
    glfw: glfw::Glfw,
    cursor_pos: (usize, usize), // Note that cursor_pos is always the location of the next
}

struct Renderer {
    font_shader: Shader,
    font_characters: Rc<RefCell<HashMap<char, Character>>>,
    font_vao: u32,
    font_vbo: u32,
    cursor_shader: Shader,
    cursor_vbo: u32,
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
        ft::FT_Set_Pixel_Sizes(face, 0, 48);

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

fn render_text(terminal: &TerminalState, renderer: &Renderer, scale: f32, color: glm::Vec3) {
    renderer.font_shader.use_shader();
    unsafe {
        gl::Enable(gl::CULL_FACE);
    };

    let mut x: f32 = 0.0;
    let mut nlines = 1;

    let uniform_color_var_name =
        std::ffi::CString::new("textColor").expect("Could not create C string.");

    unsafe {
        gl::Uniform3f(
            gl::GetUniformLocation(
                *renderer.font_shader.get_id(),
                uniform_color_var_name.as_ptr(),
            ),
            color.x,
            color.y,
            color.z,
        );

        gl::ActiveTexture(gl::TEXTURE0);
        gl::BindVertexArray(renderer.font_vao);
        
        let chars = renderer.font_characters.borrow();
        for c in terminal.buffer.chars() {
            let ch: &Character = chars.get(&c).unwrap();
            
            let w: f32 = ch.size.0 as f32 * scale;
            let h: f32 = ch.size.1 as f32 * scale;
            let (window_width, window_height) = terminal.window.borrow().get_size();

            if (x + w) > window_width as f32 {
                x = 0.0;
                nlines += 1;
            }

            let y = window_height as f32 - ((47.0 * scale) * nlines as f32) as f32; // 47 is the largest
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
            gl::BindBuffer(gl::ARRAY_BUFFER, renderer.font_vbo);
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

#[allow(unused)]
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

/**
 * Calculate the translation matrix for a cell in the grid.
 */
fn calculate_translation_matrix(
    row: usize,
    col: usize,
    nrows: usize,
    ncols: usize,
    window_width: f32,
    window_height: f32,
) -> [[f32; 4]; 4] {
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
        [ndc_x, ndc_y, 0.0, 1.0],
    ]
}

fn make_cursor_vao_vbo_ebo() -> (u32, u32) {
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
        gl::GenVertexArrays(1, &mut vao);
        gl::GenBuffers(1, &mut vbo);
        gl::GenBuffers(1, &mut ebo);
        gl::BindVertexArray(vao);

        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (std::mem::size_of::<f32>() * vertices.len()) as isize,
            vertices.as_ptr() as *const c_void,
            gl::STATIC_DRAW,
        );

        gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ebo);
        gl::BufferData(
            gl::ELEMENT_ARRAY_BUFFER,
            (std::mem::size_of::<f32>() * indices.len()) as isize,
            indices.as_ptr() as *const c_void,
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

        gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        gl::BindVertexArray(0);
    }

    (vao, vbo)
}

fn calculate_cursor_vertices(window_width: f32, window_height: f32, cell: (usize, usize)) -> ([f32; 12], [u32; 6]) {
   let (row, col) = cell;

    // Calculate cell size in normalized coordinates
    let cell_width = 2.0 / 80 as f32;
    let cell_height = 2.0 / 24 as f32;

    // Calculate bottom-left corner in normalized coordinates
    let x = -1.0 + col as f32 * cell_width;
    let y = 1.0 - (row as f32 + 1.0) * cell_height;

    // Create the vertex positions for the cell
    let vertices = [
        x, y + cell_height, 0.0,        // Top left
        x + cell_width, y + cell_height, 0.0, // Top right
        x, y, 0.0,                      // Bottom left
        x + cell_width, y, 0.0,         // Bottom right
    ];

    // Define the indices for two triangles forming a rectangle
    let indices = [
        0, 1, 2, // First triangle
        1, 2, 3, // Second triangle
    ];

    (vertices, indices)
}

fn set_renderer_vertices(renderer: &Renderer, vertices: &[f32], _indices: &[u32]) {
    unsafe {
        gl::BindBuffer(gl::ARRAY_BUFFER, renderer.cursor_vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (std::mem::size_of::<f32>() * vertices.len()) as isize,
            vertices.as_ptr() as *const c_void,
            gl::STATIC_DRAW,
        );
        // gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, renderer.ebo);
        // gl::BufferData(
        //     gl::ELEMENT_ARRAY_BUFFER,
        //     (std::mem::size_of::<u32>() * indices.len()) as isize,
        //     indices.as_ptr() as *const c_void,
        //     gl::STATIC_DRAW,
        // );
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

fn init_shaders(dir: &std::path::Path, window_width: f32, window_height: f32) -> (Shader, Shader) {
    let font_shader = Shader::new(
        dir.join("font_shader.vs").to_str().unwrap(),
        dir.join("font_shader.fs").to_str().unwrap(),
    );
    font_shader.use_shader();
    set_uniform_mat4(
        &font_shader,
        CString::new("projection").unwrap(),
        glm::ortho(0.0, window_width, 0.0, window_height, -1.0, 1.0).into(),
    );

    let cursor_shader = Shader::new(
        dir.join("cursor_shader.vs").to_str().unwrap(),
        dir.join("cursor_shader.fs").to_str().unwrap(),
    );
    cursor_shader.use_shader();

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
    let (font_shader, cursor_shader) = init_shaders(&dir, 800.0, 600.0);
    let (lib, face, characters) =
        init_freetype("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf");
    let (font_vao, font_vbo) = unsafe { make_text_vao_vbo() };
    let (_, cursor_vbo) = make_cursor_vao_vbo_ebo();

    // Set up window callbacks
    window.borrow_mut().set_framebuffer_size_callback({
        let font_shader = font_shader.clone();
        move |_window, width, height| unsafe {
            gl::Viewport(0, 0, width.into(), height.into());
            set_uniform_mat4(
                &font_shader,
                CString::new("projection").unwrap(),
                glm::ortho(0.0, width as f32, 0.0, height as f32, -1.0, 1.0).into(),
            );
        }
    });
    
    let mut ws = WindowState::new(800.0, 600.0);
    let app = AppState {
        ts: TerminalState {
            buffer: String::new(),
            cursor_pos: (0, 0),
            glfw,
            events,
            window: window.to_owned(),
        },
        ws: WindowState::new(800.0, 600.0),
        renderer: Renderer {
            font_vao,
            font_vbo,
            cursor_vbo,
            font_shader,
            font_characters: characters.clone(),
            cursor_shader,
        },
    };

    window.borrow_mut().set_key_callback({
        // let chars = characters.clone();
        move |_window, key, _scancode, action, _modifiers| {
            if let Some(key_pressed) = key_to_char(key) {
                if action == glfw::Action::Press {
                    let ch: &Character = characters.borrow().get(&key_pressed).unwrap();
                    let scale = 1.0;

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
            },
            glfw::WindowEvent::Key(_, _, Action::Press | Action::Repeat, _) => {
                app.ws.get_next_cell();
                let (vbo, ebo) = calculate_cursor_vertices(app.ws.width, app.ws.height, app.ws.next_cell);
                set_renderer_vertices(&app.renderer, &vbo, &ebo);
            },
            _ => {}
        }
    }

    unsafe {
        gl::ClearColor(0.0, 0.0, 0.0, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);

        // render_text(terminal, renderer, 0.5, glm::vec3(0.5, 0.8, 0.2));
        render_cursor(
            &app.renderer.cursor_shader,
            app.renderer.cursor_vbo,
        );
    }
}

fn main() {
    let mut app = init();

    check_gl_errors();
    while !app.ts.window.borrow().should_close() {
        tick(&mut app);
    }
}
