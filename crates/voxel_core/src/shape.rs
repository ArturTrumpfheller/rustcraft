//! Shared selection/render shape: small boxes with an orthonormal local frame.
//! Static geometry only; no allocation or Bevy types in ray/selection queries.
use crate::Block;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionBox {
    pub min: [f64; 3],
    pub max: [f64; 3],
}
impl SelectionBox {
    pub fn corners(self) -> [[f64; 3]; 8] {
        std::array::from_fn(|i| {
            std::array::from_fn(|a| {
                if i & (1 << a) == 0 {
                    self.min[a]
                } else {
                    self.max[a]
                }
            })
        })
    }
}
const CUBE: [SelectionBox; 1] = [SelectionBox {
    min: [0.; 3],
    max: [1.; 3],
}];
pub const TORCH_BOXES: [SelectionBox; 2] = [
    SelectionBox {
        min: [0.4375, 0., 0.4375],
        max: [0.5625, 0.5, 0.5625],
    },
    SelectionBox {
        min: [0.40625, 0.5, 0.40625],
        max: [0.59375, 0.6875, 0.59375],
    },
];

/// Columns are an orthonormal basis; inverse operations use its transpose.
#[derive(Clone, Copy, Debug)]
pub struct ShapeTransform {
    origin: [f64; 3],
    axes: [[f64; 3]; 3],
}
impl ShapeTransform {
    pub fn point(self, p: [f64; 3]) -> [f64; 3] {
        let v = self.vector(p);
        std::array::from_fn(|a| self.origin[a] + v[a])
    }
    pub fn vector(self, v: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|a| (0..3).map(|i| self.axes[i][a] * v[i]).sum())
    }
    pub fn inverse_point(self, p: [f64; 3]) -> [f64; 3] {
        self.inverse_vector(std::array::from_fn(|a| p[a] - self.origin[a]))
    }
    pub fn inverse_vector(self, v: [f64; 3]) -> [f64; 3] {
        self.axes.map(|axis| (0..3).map(|a| axis[a] * v[a]).sum())
    }
}
impl Block {
    pub fn selection_boxes(self) -> &'static [SelectionBox] {
        if self == Self::Air {
            &[]
        } else if self.is_plant() {
            self.plant_selection()
        } else if self.is_torch() {
            &TORCH_BOXES
        } else {
            &CUBE
        }
    }
    /// Standing geometry is unchanged. Wall models tilt ~22 degrees outward;
    /// their shaft touches the supporting plane and stays inside its own cell.
    pub fn shape_transform(self) -> ShapeTransform {
        let s = 0.375;
        let c = 0.9270248108869579; // sqrt(1 - s*s)
        let (base, axes) = match self {
            Self::TorchEast => (
                [c * 0.0625, 0.1875, 0.5],
                [[c, -s, 0.], [s, c, 0.], [0., 0., 1.]],
            ),
            Self::TorchWest => (
                [1. - c * 0.0625, 0.1875, 0.5],
                [[-c, -s, 0.], [-s, c, 0.], [0., 0., -1.]],
            ),
            Self::TorchSouth => (
                [0.5, 0.1875, c * 0.0625],
                [[0., -s, c], [0., c, s], [-1., 0., 0.]],
            ),
            Self::TorchNorth => (
                [0.5, 0.1875, 1. - c * 0.0625],
                [[0., -s, -c], [0., c, -s], [1., 0., 0.]],
            ),
            _ => {
                return ShapeTransform {
                    origin: [0.; 3],
                    axes: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
                };
            }
        };
        ShapeTransform {
            origin: std::array::from_fn(|a| base[a] - 0.5 * (axes[0][a] + axes[2][a])),
            axes,
        }
    }
}
