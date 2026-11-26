fn main() {
    let steps: i64 = std::env::args().nth(1).expect("usage: nbody STEPS").parse().expect("N");
    let pi: f64 = 3.141592653589793;
    let solar = 4.0 * pi * pi;
    let dpy = 365.24;
    let mut x: [f64; 5] = [0.0, 4.84143144246472090e+00, 8.34336671824457987e+00, 1.28943695621391310e+01, 1.53796971148509165e+01];
    let mut y: [f64; 5] = [0.0, -1.16032004402742839e+00, 4.12479856412430479e+00, -1.51111514016986312e+01, -2.59193146099879641e+01];
    let mut z: [f64; 5] = [0.0, -1.03622044471123109e-01, -4.03523417114321381e-01, -2.23307578892655734e-01, 1.79258772950371181e-01];
    let mut vx: [f64; 5] = [0.0, 1.66007664274403694e-03 * dpy, -2.76742510726862411e-03 * dpy, 2.96460137564761618e-03 * dpy, 2.68067772490389322e-03 * dpy];
    let mut vy: [f64; 5] = [0.0, 7.69901118419740425e-03 * dpy, 4.99852801234917238e-03 * dpy, 2.37847173959480950e-03 * dpy, 1.62824170038242295e-03 * dpy];
    let mut vz: [f64; 5] = [0.0, -6.90460016972063023e-05 * dpy, 2.30417297573763929e-05 * dpy, -2.96589568540237556e-05 * dpy, -9.51592254519715870e-05 * dpy];
    let m = [solar, 9.54791938424326609e-04 * solar, 2.85885980666130812e-04 * solar, 4.36624404335156298e-05 * solar, 5.15138902046611451e-05 * solar];
    let nb: usize = 5;
    let mut px = 0.0;
    let mut py = 0.0;
    let mut pz = 0.0;
    for i in 0..nb {
        px += vx[i] * m[i];
        py += vy[i] * m[i];
        pz += vz[i] * m[i];
    }
    vx[0] = -px / solar;
    vy[0] = -py / solar;
    vz[0] = -pz / solar;
    let dt: f64 = 0.01;
    for _ in 0..steps {
        for i in 0..nb {
            for j in (i + 1)..nb {
                let dx = x[i] - x[j];
                let dy = y[i] - y[j];
                let dz = z[i] - z[j];
                let d2 = dx * dx + dy * dy + dz * dz;
                let mag = dt / (d2 * d2.sqrt());
                vx[i] -= dx * m[j] * mag;
                vy[i] -= dy * m[j] * mag;
                vz[i] -= dz * m[j] * mag;
                vx[j] += dx * m[i] * mag;
                vy[j] += dy * m[i] * mag;
                vz[j] += dz * m[i] * mag;
            }
        }
        for i in 0..nb {
            x[i] += dt * vx[i];
            y[i] += dt * vy[i];
            z[i] += dt * vz[i];
        }
    }
    let mut e: f64 = 0.0;
    for i in 0..nb {
        e += 0.5 * m[i] * (vx[i] * vx[i] + vy[i] * vy[i] + vz[i] * vz[i]);
        for j in (i + 1)..nb {
            let dx = x[i] - x[j];
            let dy = y[i] - y[j];
            let dz = z[i] - z[j];
            e -= m[i] * m[j] / (dx * dx + dy * dy + dz * dz).sqrt();
        }
    }
    println!("{:?}", e);
}
