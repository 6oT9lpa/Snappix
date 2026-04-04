//! Position types for canvas and blueprint editors.

use serde::{Deserialize, Serialize};

/// 2D position for elements on canvas or blueprint.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

impl Position {
    /// Create a new position.
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Create a position at origin (0, 0).
    pub fn origin() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    /// Calculate distance to another position.
    pub fn distance_to(&self, other: &Position) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Add offset to position.
    pub fn offset(&self, dx: f32, dy: f32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }
}

impl Default for Position {
    fn default() -> Self {
        Self::origin()
    }
}

/// Rectangle area for selection and bounds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    /// Create a new rectangle.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create a rectangle from two corner positions.
    pub fn from_corners(p1: &Position, p2: &Position) -> Self {
        let x = p1.x.min(p2.x);
        let y = p1.y.min(p2.y);
        let width = (p2.x - p1.x).abs();
        let height = (p2.y - p1.y).abs();
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Check if a point is inside the rectangle.
    pub fn contains(&self, pos: &Position) -> bool {
        pos.x >= self.x
            && pos.x <= self.x + self.width
            && pos.y >= self.y
            && pos.y <= self.y + self.height
    }

    /// Get the center of the rectangle.
    pub fn center(&self) -> Position {
        Position::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Get the top-left corner.
    pub fn top_left(&self) -> Position {
        Position::new(self.x, self.y)
    }

    /// Get the bottom-right corner.
    pub fn bottom_right(&self) -> Position {
        Position::new(self.x + self.width, self.y + self.height)
    }
}

impl Default for Rect {
    fn default() -> Self {
        Self::new(0.0, 0.0, 100.0, 100.0)
    }
}
