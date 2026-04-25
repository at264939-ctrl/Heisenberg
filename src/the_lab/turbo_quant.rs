// the_lab/turbo_quant.rs -- Implementation of TurboQuant algorithms from the paper
// "TurboQuant: Online Vector Quantization with Near-optimal Distortion Rate"

#![allow(dead_code)]

use anyhow::Result;
use nalgebra::{DMatrix, DVector};
use rand::distributions::Distribution;
use rand::thread_rng;
use rand_distr::{Normal, StandardNormal};

/// Calculates the optimal k-means centroids for the (shifted) Beta distribution
/// For extreme high dimensions we approximate with N(0, 1/d).
fn generate_mse_codebook(d: usize, b: usize) -> Vec<f64> {
    let num_centroids = 1_usize << b;
    
    // For small bit-widths we use precomputed values from the paper
    // "the optimal quantization centroids for bit-widths b=1, 2 are ±0.453/sqrt(d), ±1.51/sqrt(d)"
    if b == 1 {
        let val = (2.0 / std::f64::consts::PI).sqrt() / (d as f64).sqrt();
        return vec![-val, val];
    }
    if b == 2 {
        let val1 = 0.453 / (d as f64).sqrt();
        let val2 = 1.51 / (d as f64).sqrt();
        return vec![-val2, -val1, val1, val2];
    }
    
    // Fallback: Uniform splitting of the expected range [-3/sqrt(d), 3/sqrt(d)]
    let mut codebook = Vec::with_capacity(num_centroids);
    let scale = 3.0 / (d as f64).sqrt();
    let step = (2.0 * scale) / (num_centroids as f64 - 1.0).max(1.0);
    
    for i in 0..num_centroids {
        codebook.push(-scale + i as f64 * step);
    }
    codebook
}

/// Algorithm 1: TurboQuantmse
pub struct TurboQuantMse {
    d: usize,
    b: usize,
    pub pi: DMatrix<f64>, // Random rotation matrix
    pub pi_t: DMatrix<f64>, // Transpose for dequant
    pub codebook: Vec<f64>,
}

impl TurboQuantMse {
    pub fn new(d: usize, b: usize) -> Self {
        let mut rng = thread_rng();
        let normal = Normal::new(0.0, 1.0).unwrap();
        
        // Generate random matrix
        let mut mat = DMatrix::zeros(d, d);
        for i in 0..d {
            for j in 0..d {
                mat[(i, j)] = normal.sample(&mut rng);
            }
        }
        
        // Apply QR decomposition to get random rotation matrix Π (Q)
        let qr = mat.qr();
        let pi = qr.q();
        let pi_t = pi.transpose();
        
        let codebook = generate_mse_codebook(d, b);
        
        Self { d, b, pi, pi_t, codebook }
    }
    
    pub fn quant_mse(&self, x: &DVector<f64>) -> Vec<usize> {
        let y = &self.pi * x;
        let mut idx = Vec::with_capacity(self.d);
        
        for i in 0..self.d {
            let val = y[i];
            let mut best_idx = 0;
            let mut best_dist = f64::MAX;
            for (curr_idx, &c) in self.codebook.iter().enumerate() {
                let dist = (val - c).abs();
                if dist < best_dist {
                    best_dist = dist;
                    best_idx = curr_idx;
                }
            }
            idx.push(best_idx);
        }
        idx
    }
    
    pub fn dequant_mse(&self, idx: &[usize]) -> DVector<f64> {
        let mut y_tilde = DVector::zeros(self.d);
        for i in 0..self.d {
            y_tilde[i] = self.codebook[idx[i]];
        }
        &self.pi_t * y_tilde
    }
}

/// Algorithm 2: TurboQuantprod (optimized for inner product)
pub struct TurboQuantProd {
    d: usize,
    mse_quant: TurboQuantMse,
    s: DMatrix<f64>, // random projection matrix for QJL
    s_t: DMatrix<f64>,
}

pub struct ProdQuantResult {
    pub idx: Vec<usize>,
    pub qjl: DVector<f64>, // +/- 1.0 values
    pub gamma: f64, // ||r||_2
}

impl TurboQuantProd {
    pub fn new(d: usize, b: usize) -> Self {
        // Instantiate TurboQuantmse with bit-width b - 1
        let mse_quant = TurboQuantMse::new(d, b.saturating_sub(1).max(1));
        
        let mut rng = thread_rng();
        let mut s = DMatrix::zeros(d, d);
        for i in 0..d {
            for j in 0..d {
                s[(i, j)] = StandardNormal.sample(&mut rng);
            }
        }
        let s_t = s.transpose();
        
        Self { d, mse_quant, s, s_t }
    }
    
    pub fn quant_prod(&self, x: &DVector<f64>) -> ProdQuantResult {
        let idx = self.mse_quant.quant_mse(x);
        let x_tilde = self.mse_quant.dequant_mse(&idx);
        let r = x - x_tilde;
        
        let gamma = r.norm(); // L2 norm
        let qjl_vec = &self.s * r;
        
        let mut qjl = DVector::zeros(self.d);
        for i in 0..self.d {
            qjl[i] = if qjl_vec[i] >= 0.0 { 1.0 } else { -1.0 };
        }
        
        ProdQuantResult { idx, qjl, gamma }
    }
    
    pub fn dequant_prod(&self, res: &ProdQuantResult) -> DVector<f64> {
        let x_mse = self.mse_quant.dequant_mse(&res.idx);
        
        let pi_factor = (std::f64::consts::PI / 2.0).sqrt();
        let scale = pi_factor / (self.d as f64);
        
        let x_qjl = scale * res.gamma * (&self.s_t * &res.qjl);
        
        x_mse + x_qjl
    }
}
