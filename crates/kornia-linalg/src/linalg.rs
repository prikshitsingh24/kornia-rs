use std::ops::{Index, IndexMut};

use glam::{Mat3, Quat, Vec3};

const GEMMA: f32 = 5.828427124;
const CSTAR: f32 = 0.923879532;
const SSTAR: f32 = 0.3826834323;
const SVD_EPSILON: f32 = 1e-6;
const JACOBI_STEPS: u32 = 12;
const RSQRT_STEPS: u32 = 4;
const RSQRT1_STEPS: u32 = 6;

/// Calculates the result of x / y. Required as the accurate square root function otherwise uses a reciprocal approximation when using optimizations on a GPU which can lead to slightly different results. If non exact matching results are acceptable a simple division can be used.
pub fn fdiv(x: f32, y: f32) -> f32 {
    return x / y;
}

/// Calculates the reciprocal square root of x using a fast approximation.
pub fn rsqrt(x: f32) -> f32 {
    let xhalf = -0.5 * x;
    let mut i = x.to_bits() as i32; // Convert float to raw bits
    i = 0x5f375a82 - (i >> 1); // Magic constant and bit manipulation
    let mut x = f32::from_bits(i as u32); // Convert bits back to float

    for _ in 0..RSQRT_STEPS {
        x = x * (x * x * xhalf + 1.5);
    }

    x
}

/// See rsqrt. Uses RSQRT1_STEPS to offer a higher precision alternative
pub fn rsqrt1(mut x: f32) -> f32 {
    let xhalf = -0.5 * x;
    let mut i = x.to_bits() as i32;
    i = 0x5f37599e - (i >> 1);
    x = f32::from_bits(i as u32);

    for _ in 0..RSQRT1_STEPS {
        x = x * (x * x * xhalf + 1.5);
    }

    return x;
}

/// Calculates the square root of x using 1.f/rsqrt1(x) to give a square root with controllable and consistent precision.
pub fn accurate_sqrt(x: f32) -> f32 {
    return fdiv(1.0, rsqrt(x));
}

/// Helper function used to swap X with Y and Y with  X if c == true
fn cond_swap(c: bool, x: &mut f32, y: &mut f32) {
    let z = *x;
    if c {
        *x = *y;
        *y = z;
    }
}

// Helper function to swap X and Y and swap Y with -X if c is true
fn cond_neg_swap(c: bool, x: &mut f32, y: &mut f32) {
    let z = -(*x);
    if c {
        *x = *y;
        *y = z;
    }
}

/// Helper function used to convert quaternion to matrix
pub fn quaternion_to_matrix(q: &IndexedQuat) -> Mat3 {
    let w = q[3];
    let x = q[0];
    let y = q[1];
    let z = q[2];

    // Return a Mat3 constructed with the proper rows
    Mat3 {
        x_axis: Vec3::new(
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y - w * z),
            2.0 * (x * z + w * y),
        ),
        y_axis: Vec3::new(
            2.0 * (x * y + w * z),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z - w * x),
        ),
        z_axis: Vec3::new(
            2.0 * (x * z - w * y),
            2.0 * (y * z + w * x),
            1.0 - 2.0 * (x * x + y * y),
        ),
    }
}

#[derive(Debug, Clone, Copy)]
/// A simple symmetric 3x3 Matrix class (contains no storage for (0, 1) (0, 2) and (1, 2)
pub struct Symmetric3x3 {
    /// The element at row 0, column 0 of the matrix, typically the first diagonal element.
    pub m_00: f32,

    /// The element at row 1, column 0 of the matrix. Since this is a symmetric matrix, it is equivalent to `m_01`.
    pub m_10: f32,

    /// The element at row 1, column 1 of the matrix, the second diagonal element.
    pub m_11: f32,

    /// The element at row 2, column 0 of the matrix. Since this is a symmetric matrix, it is equivalent to `m_02`.
    pub m_20: f32,

    /// The element at row 2, column 1 of the matrix. Since this is a symmetric matrix, it is equivalent to `m_12`.
    pub m_21: f32,

    /// The element at row 2, column 2 of the matrix, the third diagonal element.
    pub m_22: f32,
}

impl Symmetric3x3 {
    /// Constructor to initialize the symmetric matrix with given values
    pub fn new(a11: f32, a21: f32, a22: f32, a31: f32, a32: f32, a33: f32) -> Self {
        Symmetric3x3 {
            m_00: a11,
            m_10: a21,
            m_11: a22,
            m_20: a31,
            m_21: a32,
            m_22: a33,
        }
    }

    /// Constructor from a regular Mat3x3 (assuming Mat3x3 exists)
    pub fn from_mat3x3(mat: &Mat3) -> Self {
        Symmetric3x3 {
            m_00: mat.x_axis.x,
            m_10: mat.x_axis.y,
            m_11: mat.y_axis.y,
            m_20: mat.z_axis.x,
            m_21: mat.z_axis.y,
            m_22: mat.z_axis.z,
        }
    }
}

#[derive(Debug, Clone, Copy)]
/// Helper struct to store 2 floats to avoid OUT parameters on functions
pub struct Givens {
    /// The cosine of the angle in the Givens rotation.
    pub ch: f32,

    /// The sine of the angle in the Givens rotation.
    pub sh: f32,
}

impl Givens {
    /// Constructor with default values for ch and sh
    pub fn new(ch: f32, sh: f32) -> Self {
        Givens { ch, sh }
    }

    /// Constructor with default CSTAR and SSTAR values
    pub fn default() -> Self {
        Givens {
            ch: CSTAR,
            sh: SSTAR,
        }
    }
}

#[derive(Debug, Clone, Copy)]
/// Helper struct to store 2 Matrices to avoid OUT parameters on functions
pub struct QR {
    /// The orthogonal matrix Q from the QR decomposition.
    pub Q: Mat3,

    /// The upper triangular matrix R from the QR decomposition.
    pub R: Mat3,
}

#[derive(Debug, Clone, Copy)]
/// Helper struct to store 3 Matrices to avoid OUT parameters on functions
pub struct SVDSet {
    /// The matrix of left singular vectors.
    pub U: Mat3,

    /// The diagonal matrix of singular values.
    pub S: Mat3,

    /// The matrix of right singular vectors.
    pub V: Mat3,
}

/// Calculates the squared norm of the vector [x y z] using a standard scalar product d = x * x + y *y + z * z
pub fn dist2(x: f32, y: f32, z: f32) -> f32 {
    x * x + (y * y + z * z)
}

/// For an explanation of the math see http://pages.cs.wisc.edu/~sifakis/papers/SVD_TR1690.pdf
/// Computing the Singular Value Decomposition of 3 x 3 matrices with minimal branching and elementary floating point operations
/// See Algorithm 2 in reference. Given a matrix A this function returns the givens quaternion (x and w component, y and z are 0)
pub fn approximate_givens_quaternion(A: &Symmetric3x3) -> Givens {
    let g = Givens {
        ch: 2.0 * (A.m_00 - A.m_11),
        sh: A.m_10,
    };

    let mut b = GEMMA * g.sh * g.sh < g.ch * g.ch;
    let w = rsqrt(g.ch * g.ch + g.sh * g.sh);

    if w != w {
        // Checking for NaN
        b = false;
    }

    Givens {
        ch: if b { w * g.ch } else { CSTAR },
        sh: if b { w * g.sh } else { SSTAR },
    }
}

#[derive(Debug)]
/// A wrapper around the `glam::Quat` type that allows dynamic indexing into its components.
///
/// This struct provides custom indexing behavior for quaternion components (`x`, `y`, `z`, and `w`),
/// enabling access and mutation using an index (e.g., `q[0]`, `q[1]`, etc.). It implements both
/// the `Index` and `IndexMut` traits to allow for immutable and mutable access to the quaternion's components.
pub struct IndexedQuat(Quat);

impl IndexedQuat {
    fn new(q: Quat) -> Self {
        IndexedQuat(q)
    }
}

impl Index<usize> for IndexedQuat {
    type Output = f32;

    fn index(&self, index: usize) -> &Self::Output {
        match index {
            0 => &self.0.x,
            1 => &self.0.y,
            2 => &self.0.z,
            3 => &self.0.w,
            _ => panic!("Index out of bounds for Quaternion"),
        }
    }
}

impl IndexMut<usize> for IndexedQuat {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        match index {
            0 => &mut self.0.x,
            1 => &mut self.0.y,
            2 => &mut self.0.z,
            3 => &mut self.0.w,
            _ => panic!("Index out of bounds for Quaternion"),
        }
    }
}

/// Function used to apply a givens rotation S. Calculates the weights and updates the quaternion to contain the cumultative rotation
pub fn jacobi_conjugation(x: usize, y: usize, z: usize, S: &mut Symmetric3x3, q: &mut IndexedQuat) {
    // Compute the Givens rotation (approximated)
    let mut g = approximate_givens_quaternion(S);
    // Scale and calculate intermediate values
    let scale = 1.0 / (g.ch * g.ch + g.sh * g.sh);
    let a = (g.ch * g.ch - g.sh * g.sh) * scale;
    let b = 2.0 * g.sh * g.ch * scale;

    // Create a copy of the matrix to avoid modifying the original during calculations
    let mut _S = S.clone();

    // Perform conjugation: S = Q'*S*Q
    S.m_00 = a * (a * _S.m_00 + b * _S.m_10) + b * (a * _S.m_10 + b * _S.m_11);
    S.m_10 = a * (-b * _S.m_00 + a * _S.m_10) + b * (-b * _S.m_10 + a * _S.m_11);
    S.m_11 = -b * (-b * _S.m_00 + a * _S.m_10) + a * (-b * _S.m_10 + a * _S.m_11);
    S.m_20 = a * _S.m_20 + b * _S.m_21;
    S.m_21 = -b * _S.m_20 + a * _S.m_21;
    S.m_22 = _S.m_22;

    // Update cumulative rotation qV
    let mut tmp = [0.0, 0.0, 0.0];
    tmp[0] = q[0] * g.sh;
    tmp[1] = q[1] * g.sh;
    tmp[2] = q[2] * g.sh;
    g.sh *= q[3];

    // (x, y, z) corresponds to (0,1,2), (1,2,0), (2,0,1) for (p, q) = (0,1), (1,2), (0,2)
    q[z] = q[z] * g.ch + g.sh;
    q[3] = q[3] * g.ch - tmp[z]; // w
    q[x] = q[x] * g.ch + tmp[y];
    q[y] = q[y] * g.ch - tmp[x];

    // Re-arrange matrix for next iteration
    _S.m_00 = S.m_11;
    _S.m_10 = S.m_21;
    _S.m_11 = S.m_22;
    _S.m_20 = S.m_10;
    _S.m_21 = S.m_20;
    _S.m_22 = S.m_00;

    S.m_00 = _S.m_00;
    S.m_10 = _S.m_10;
    S.m_11 = _S.m_11;
    S.m_20 = _S.m_20;
    S.m_21 = _S.m_21;
    S.m_22 = _S.m_22;
}

/// Function used to contain the givens permutations and the loop of the jacobi steps controlled by JACOBI_STEPS
/// Returns the quaternion q containing the cumultative result used to reconstruct S
pub fn jacobi_eigenanalysis(mut S: Symmetric3x3) -> Mat3 {
    let mut q = IndexedQuat::new(Quat::from_xyzw(0.0, 0.0, 0.0, 1.0));
    for _i in 0..JACOBI_STEPS {
        jacobi_conjugation(0, 1, 2, &mut S, &mut q);
        jacobi_conjugation(1, 2, 0, &mut S, &mut q);
        jacobi_conjugation(2, 0, 1, &mut S, &mut q);
    }
    return quaternion_to_matrix(&q);
}

/// Implementation of Algorithm 3
pub fn sort_singular_values(B: &mut Mat3, V: &mut Mat3) {
    let mut rho1 = dist2(B.x_axis.x, B.y_axis.x, B.z_axis.x);
    let mut rho2 = dist2(B.x_axis.y, B.y_axis.y, B.z_axis.y);
    let mut rho3 = dist2(B.x_axis.z, B.y_axis.z, B.z_axis.z);

    let mut c = rho1 < rho2;
    cond_neg_swap(c, &mut B.x_axis.x, &mut B.x_axis.y);
    cond_neg_swap(c, &mut V.x_axis.x, &mut V.x_axis.y);
    cond_neg_swap(c, &mut B.y_axis.x, &mut B.y_axis.y);
    cond_neg_swap(c, &mut V.y_axis.x, &mut V.y_axis.y);
    cond_neg_swap(c, &mut B.z_axis.x, &mut B.z_axis.y);
    cond_neg_swap(c, &mut V.z_axis.x, &mut V.z_axis.y);
    cond_swap(c, &mut rho1, &mut rho2);

    c = rho1 < rho3;
    cond_neg_swap(c, &mut B.x_axis.x, &mut B.x_axis.z);
    cond_neg_swap(c, &mut V.x_axis.x, &mut V.x_axis.z);
    cond_neg_swap(c, &mut B.y_axis.x, &mut B.y_axis.z);
    cond_neg_swap(c, &mut V.y_axis.x, &mut V.y_axis.z);
    cond_neg_swap(c, &mut B.z_axis.x, &mut B.z_axis.z);
    cond_neg_swap(c, &mut V.z_axis.x, &mut V.z_axis.z);
    cond_swap(c, &mut rho1, &mut rho3);

    c = rho2 < rho3;
    cond_neg_swap(c, &mut B.x_axis.y, &mut B.x_axis.z);
    cond_neg_swap(c, &mut V.x_axis.y, &mut V.x_axis.z);
    cond_neg_swap(c, &mut B.y_axis.y, &mut B.y_axis.z);
    cond_neg_swap(c, &mut V.y_axis.y, &mut V.y_axis.z);
    cond_neg_swap(c, &mut B.z_axis.y, &mut B.z_axis.z);
    cond_neg_swap(c, &mut V.z_axis.y, &mut V.z_axis.z);
}

/// Implementation of Algorithm 4
pub fn qr_givens_quaternion(a1: f32, a2: f32) -> Givens {
    // a1 = pivot point on diagonal
    // a2 = lower triangular entry we want to annihilate
    let epsilon = SVD_EPSILON; // Assuming _SVD_EPSILON is defined elsewhere
    let rho = accurate_sqrt(a1 * a1 + a2 * a2);

    let mut g = Givens {
        ch: (a1.abs() + (f32::max(rho, epsilon))),
        sh: if rho > epsilon { a2 } else { 0.0 },
    };

    let b = a1 < 0.0;
    cond_swap(b, &mut g.sh, &mut g.ch);

    let w = rsqrt(g.ch * g.ch + g.sh * g.sh);
    g.ch *= w;
    g.sh *= w;

    return g;
}

/// Implements a QR decomposition of a Matrix
pub fn qr_decomposition(B: &mut Mat3) -> QR {
    let mut Q = Mat3::ZERO;
    let mut R = Mat3::ZERO;

    // First Givens rotation (ch, 0, 0, sh)
    let g1 = qr_givens_quaternion(B.x_axis.x, B.y_axis.x);
    let mut a = -2.0 * g1.sh * g1.sh + 1.0;
    let mut b = 2.0 * g1.ch * g1.sh;

    // Apply B = Q' * B
    R.x_axis.x = a * B.x_axis.x + b * B.y_axis.x;
    R.x_axis.y = a * B.x_axis.y + b * B.y_axis.y;
    R.x_axis.z = a * B.x_axis.z + b * B.y_axis.z;
    R.y_axis.x = -b * B.x_axis.x + a * B.y_axis.x;
    R.y_axis.y = -b * B.x_axis.y + a * B.y_axis.y;
    R.y_axis.z = -b * B.x_axis.z + a * B.y_axis.z;
    R.z_axis.x = B.z_axis.x;
    R.z_axis.y = B.z_axis.y;
    R.z_axis.z = B.z_axis.z;

    // Second Givens rotation (ch, 0, -sh, 0)
    let g2 = qr_givens_quaternion(R.x_axis.x, R.z_axis.x);
    a = -2.0 * g2.sh * g2.sh + 1.0;
    b = 2.0 * g2.ch * g2.sh;

    // Apply B = Q' * B
    B.x_axis.x = a * R.x_axis.x + b * R.z_axis.x;
    B.x_axis.y = a * R.x_axis.y + b * R.z_axis.y;
    B.x_axis.z = a * R.x_axis.z + b * R.z_axis.z;
    B.y_axis.x = R.y_axis.x;
    B.y_axis.y = R.y_axis.y;
    B.y_axis.z = R.y_axis.z;
    B.z_axis.x = -b * R.x_axis.x + a * R.z_axis.x;
    B.z_axis.y = -b * R.x_axis.y + a * R.z_axis.y;
    B.z_axis.z = -b * R.x_axis.z + a * R.z_axis.z;

    // Third Givens rotation (ch, sh, 0, 0)
    let g3 = qr_givens_quaternion(B.y_axis.y, B.z_axis.y);
    a = -2.0 * g3.sh * g3.sh + 1.0;
    b = 2.0 * g3.ch * g3.sh;

    // R is now set to desired value
    R.x_axis.x = B.x_axis.x;
    R.x_axis.y = B.x_axis.y;
    R.x_axis.z = B.x_axis.z;
    R.y_axis.x = a * B.y_axis.x + b * B.z_axis.x;
    R.y_axis.y = a * B.y_axis.y + b * B.z_axis.y;
    R.y_axis.z = a * B.y_axis.z + b * B.z_axis.z;
    R.z_axis.x = -b * B.y_axis.x + a * B.z_axis.x;
    R.z_axis.y = -b * B.y_axis.y + a * B.z_axis.y;
    R.z_axis.z = -b * B.y_axis.z + a * B.z_axis.z;

    // Construct the cumulative rotation Q = Q1 * Q2 * Q3
    let sh12 = 2.0 * (g1.sh * g1.sh - 0.5);
    let sh22 = 2.0 * (g2.sh * g2.sh - 0.5);
    let sh32 = 2.0 * (g3.sh * g3.sh - 0.5);

    Q.x_axis.x = sh12 * sh22;
    Q.x_axis.y = 4.0 * g2.ch * g3.ch * sh12 * g2.sh * g3.sh + 2.0 * g1.ch * g1.sh * sh32;
    Q.x_axis.z = 4.0 * g1.ch * g3.ch * g1.sh * g3.sh - 2.0 * g2.ch * sh12 * g2.sh * sh32;

    Q.y_axis.x = -2.0 * g1.ch * g1.sh * sh22;
    Q.y_axis.y = -8.0 * g1.ch * g2.ch * g3.ch * g1.sh * g2.sh * g3.sh + sh12 * sh32;
    Q.y_axis.z =
        -2.0 * g3.ch * g3.sh + 4.0 * g1.sh * (g3.ch * g1.sh * g3.sh + g1.ch * g2.ch * g2.sh * sh32);

    Q.z_axis.x = 2.0 * g2.ch * g2.sh;
    Q.z_axis.y = -2.0 * g3.ch * sh22 * g3.sh;
    Q.z_axis.z = sh22 * sh32;

    QR { Q, R }
}

/// Wrapping function used to contain all of the required sub calls
pub fn svd(A: Mat3) -> SVDSet {
    // Compute the eigenvectors of A^T * A, which is V in SVD (Singular Vectors)
    let V = jacobi_eigenanalysis(Symmetric3x3::from_mat3x3(&(A.transpose().mul_mat3(&A))));

    // Compute B = A * V
    let mut B = A.mul_mat3(&V);

    // Sort the singular values
    sort_singular_values(&mut B, &mut V.clone());

    // Perform QR decomposition on B to get Q and R
    let qr = qr_decomposition(&mut B);

    // Reset MXCSR register (if needed)

    // Return the SVD result, which includes Q (as U), R (as S), and V
    SVDSet {
        U: qr.Q,
        S: qr.R,
        V,
    }
}

#[cfg(test)]
mod tests {
    use glam::{Mat3, Vec3};

    use super::*;

    #[test]
    fn test_svd() {
        // Define a simple 3x3 matrix A
        let A = Mat3 {
            x_axis: Vec3::new(1.0, 0.0, 0.0),
            y_axis: Vec3::new(0.0, 2.0, 0.0),
            z_axis: Vec3::new(0.0, 0.0, 3.0),
        };

        // Perform SVD on matrix A
        let svd_result = svd(A);
        // Check matrix V
        assert!(
            A == svd_result
                .U
                .mul_mat3(&(svd_result.S.mul_mat3(&svd_result.V))),
            "The calculated SVD is wrong"
        );

        let singular_values = vec![
            svd_result.S.x_axis.x,
            svd_result.S.y_axis.y,
            svd_result.S.z_axis.z,
        ];
        assert!(
            singular_values[0] >= singular_values[1] && singular_values[1] >= singular_values[2],
            "Singular values are not sorted properly"
        );
    }
}
