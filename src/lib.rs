use std::path::Path;

use image::{GenericImageView, ImageReader, RgbImage, SubImage};

/// abstraction over the bitmap buffer of the window, to add a width and height in screen pixels to
/// the window
pub struct Bitmap {
    buffer: Vec<u32>,
    width: usize,
    height: usize,
}

/// abstraction over the bitmap, to subdivide in into virtual pixels (for a pixelated look)
struct PixelGrid {
    bitmap: Bitmap,
    width: usize,
    height: usize,
    /// side length of a virtual pixel in screen pixels (is a float because of approximations)
    pixel_size: f64,
    /// the pixel grid is either clamped by the height or the width of the window
    clamped_by: ClampType,
    /// offset in the pixel grid in respect to the bitmap caused by the clamping,
    /// can be an offset in the x or y coordinate, depending on the clamp type
    pixel_offset: usize,
}

enum ClampType {
    Height,
    Width,
}

/// abstraction over the pixel grid, to subdivide the pixel grid into tiles, and draw images on the
/// tiles
pub struct TileGrid {
    pixel_grid: PixelGrid,
    width: usize,
    height: usize,
    // side lenght of a tile, in virtual pixels
    tile_size: usize,
    sprite_sheet: SpriteSheet,
}

struct SpriteSheet {
    image: RgbImage,
    // side lenght of a sprite, in pixels
    sprite_size: usize,
    id_to_coords: fn(sprite_id: usize) -> (usize, usize),
}

impl Bitmap {
    /// constructs a bitmap abstraction on top of a Vec<u32>
    ///
    /// # Examples
    /// ```
    /// use tiley::Bitmap;
    ///
    /// let buffer = vec![0; 600 * 200];
    /// let bitmap = Bitmap::from_vec(buffer, 600, 200);
    /// ```
    pub fn from_vec(buffer: Vec<u32>, width: usize, height: usize) -> Self {
        debug_assert!(width * height == buffer.len());

        Self {
            buffer,
            width,
            height,
        }
    }

    pub fn as_vec(&self) -> &Vec<u32> {
        &self.buffer
    }

    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// completely fills in the bitmap with a single color.
    /// useful to color the background or delete the previous frame
    ///
    /// # Examples
    /// ```
    /// use tiley::Bitmap;
    ///
    /// let mut bitmap = Bitmap::from_vec(vec![0; 600 * 200], 600, 200);
    /// bitmap.fill(0xffffff);
    /// assert!(bitmap.as_vec().iter().all(|p| *p == 0xffffff));
    /// ```
    pub fn fill(&mut self, color: u32) {
        self.buffer.iter_mut().for_each(|p| *p = color);
    }

    fn draw_pixel(&mut self, (x, y): (usize, usize), color: u32) {
        debug_assert!(x < self.width);
        debug_assert!(y < self.height);

        self.buffer[x + y * self.width] = color;
    }

    /// this will draw a rectangle on the window by specifying the top left pixel (x1, y1) and bottom
    /// right pixel (x2, y2)
    fn draw_rectangle_pixels(
        &mut self,
        (x1, y1): (usize, usize),
        (x2, y2): (usize, usize),
        color: u32,
    ) {
        debug_assert!(x1 < x2);
        debug_assert!(y1 < y2);
        debug_assert!(x2 < self.width);
        debug_assert!(y2 < self.height);

        for x in x1..=x2 {
            for y in y1..=y2 {
                self.draw_pixel((x, y), color);
            }
        }
    }
}

impl PixelGrid {
    fn new(bitmap: Bitmap, width: usize, height: usize) -> Self {
        let clamped_by =
            match (bitmap.width as f64 / width as f64) < (bitmap.height as f64 / height as f64) {
                true => ClampType::Width,
                false => ClampType::Height,
            };

        let pixel_size = match clamped_by {
            ClampType::Height => bitmap.height as f64 / height as f64,
            ClampType::Width => bitmap.width as f64 / width as f64,
        };

        let pixel_offset = match clamped_by {
            ClampType::Height => bitmap.width - (pixel_size * width as f64) as usize,
            ClampType::Width => bitmap.height - (pixel_size * height as f64) as usize,
        } / 2;

        PixelGrid {
            bitmap,
            width,
            height,
            clamped_by,
            pixel_size,
            pixel_offset,
        }
    }

    /// this will draw a "virtual" pixel in the pixel grid, which is a square in the bitmap
    fn draw_virtual_pixel(&mut self, (x, y): (usize, usize), color: u32) {
        debug_assert!(x < self.width);
        debug_assert!(y < self.height);

        // calculate the square coordinates in the bitmap
        let (x1, y1) = (self.pixel_size * x as f64, self.pixel_size * y as f64);
        let (x1, y1) = (x1 as usize, y1 as usize);

        let (x2, y2) = (
            self.pixel_size * (x + 1) as f64,
            self.pixel_size * (y + 1) as f64,
        );
        let (x2, y2) = (x2 as usize, y2 as usize);
        let (x2, y2) = (x2 - 1, y2 - 1);

        // offset caused by clamping
        let (dx, dy) = match self.clamped_by {
            ClampType::Height => (self.pixel_offset, 0),
            ClampType::Width => (0, self.pixel_offset),
        };
        let (x1, y1) = (x1 + dx, y1 + dy);
        let (x2, y2) = (x2 + dx, y2 + dy);

        self.bitmap.draw_rectangle_pixels((x1, y1), (x2, y2), color);
    }

    /// function to draw an image mapping the image pixels to the PixelGrid virtual pixels
    fn draw_image(&mut self, (x, y): (usize, usize), image: SubImage<&RgbImage>) {
        let (image_width, image_height) = image.dimensions();

        debug_assert!(x + image_width as usize - 1 < self.width);
        debug_assert!(y + image_height as usize - 1 < self.height);

        for dx in 0..image_width {
            for dy in 0..image_height {
                let color = image.get_pixel(dx, dy);
                let color = u32::from_be_bytes([0, color.0[0], color.0[1], color.0[2]]);
                self.draw_virtual_pixel((x + dx as usize, y + dy as usize), color);
            }
        }
    }
}

impl SpriteSheet {
    fn new(path: &Path, sprite_size: usize) -> Self {
        dbg!(path);
        let image = ImageReader::open(path)
            .expect("error opening the image")
            .decode()
            .expect("error decoding the image");

        SpriteSheet {
            image: image.into(),
            id_to_coords: linear_translation,
            sprite_size,
        }
    }

    fn sprite(&self, sprite_id: usize) -> SubImage<&RgbImage> {
        let (sprite_x, sprite_y) = (self.id_to_coords)(sprite_id);

        let (image_width, image_height) = self.image.dimensions();

        debug_assert!(image_width as usize > (sprite_x + 1) * self.sprite_size - 1);
        debug_assert!(image_height as usize > (sprite_y + 1) * self.sprite_size - 1);

        // cut out the subimage containing the correct sprite
        self.image.view(
            (sprite_x * self.sprite_size) as u32,
            (sprite_y * self.sprite_size) as u32,
            self.sprite_size as u32,
            self.sprite_size as u32,
        )
    }
}

fn linear_translation(sprite_id: usize) -> (usize, usize) {
    (sprite_id, 0)
}

impl TileGrid {
    /// creates a new tile grid on top of a bitmap
    ///
    /// # Examples
    ///
    /// ```
    /// use tiley::{Bitmap, TileGrid};
    ///
    /// let bitmap = Bitmap::from_vec(vec![0; 600 * 200], 600, 200);
    /// ```
    /// ```ignore
    /// let tile_grid = TileGrid::new(bitmap, 20, 30, 8, std::path::Path::new("./resources/sprite_sheet.png"));
    /// ```
    pub fn new(
        bitmap: Bitmap,
        width: usize,
        height: usize,
        tile_size: usize,
        sprite_sheet_path: &Path,
    ) -> Self {
        // calculate the pixel_grid dimensions
        let pixel_grid_width = width * tile_size;
        let pixel_grid_height = height * tile_size;

        let pixel_grid = PixelGrid::new(bitmap, pixel_grid_width, pixel_grid_height);

        let sprite_sheet = SpriteSheet::new(sprite_sheet_path, tile_size);

        Self {
            pixel_grid,
            width,
            height,
            tile_size,
            sprite_sheet,
        }
    }

    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// draws a tile in the tile coordinates, using a sprite cut from the sprite sheet on the
    /// sprite id
    pub fn draw_tile(&mut self, (tile_x, tile_y): (usize, usize), sprite_id: usize) {
        let sprite = self.sprite_sheet.sprite(sprite_id);

        debug_assert!(tile_x < self.width);
        debug_assert!(tile_y < self.height);

        // virtual pixel coordinates
        let (pixel_x, pixel_y) = (tile_x * self.tile_size, tile_y * self.tile_size);

        self.pixel_grid.draw_image((pixel_x, pixel_y), sprite);
    }
}
