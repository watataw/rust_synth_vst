pub const CABLE_NODES: usize = 16;
const GRAVITY: f32 = 1150.0;
const DAMPING: f32 = 0.982;
const CONSTRAINT_ITERATIONS: usize = 6;
const SLEEP_ENERGY_THRESHOLD: f32 = 0.004;

#[derive(Clone, Copy, Debug, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    #[inline]
    pub fn distance(self, other: Self) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }

    #[inline]
    pub fn length_sq(self) -> f32 {
        self.x * self.x + self.y * self.y
    }
}

pub const CABLE_PALETTE: [(u8, u8, u8); 6] = [
    (235, 77, 75),   // Rouge Eurorack
    (46, 204, 113),  // Vert Émeraude
    (52, 152, 219),  // Bleu Cobalt
    (241, 196, 15),  // Jaune Or
    (155, 89, 182),  // Violet
    (230, 126, 34),  // Orange Cuivre
];

pub struct CablePhysics {
    pub pos: [Vec2; CABLE_NODES],
    pub old_pos: [Vec2; CABLE_NODES],
    pub rest_segment_len: f32,
    pub color: (u8, u8, u8),
    pub sleeping: bool,
    last_start: Vec2,
    last_end: Vec2,
}

impl CablePhysics {
    pub fn new(start: Vec2, end: Vec2, color_index: usize) -> Self {
        let mut pos = [Vec2::default(); CABLE_NODES];
        let mut old_pos = [Vec2::default(); CABLE_NODES];

        let chord = start.distance(end).max(40.0);
        // Légère moue naturelle de 18 % pour que le câble pende avec grâce
        let total_len = chord * 1.18 + 35.0;
        let rest_segment_len = total_len / (CABLE_NODES - 1) as f32;

        for i in 0..CABLE_NODES {
            let t = i as f32 / (CABLE_NODES - 1) as f32;
            let sag = (t * std::f32::consts::PI).sin() * 35.0;
            let p = Vec2::new(
                start.x + (end.x - start.x) * t,
                start.y + (end.y - start.y) * t + sag,
            );
            pos[i] = p;
            old_pos[i] = p;
        }

        Self {
            pos,
            old_pos,
            rest_segment_len,
            color: CABLE_PALETTE[color_index % CABLE_PALETTE.len()],
            sleeping: false,
            last_start: start,
            last_end: end,
        }
    }

    pub fn wake(&mut self) {
        self.sleeping = false;
    }

    /// Met à jour la simulation physique pour un pas de temps `dt` (ex: 1/60s).
    pub fn step(&mut self, start: Vec2, end: Vec2, dt: f32) {
        // Réveille le câble si l'une des extrémités s'est déplacée
        if start.distance(self.last_start) > 0.05 || end.distance(self.last_end) > 0.05 {
            self.sleeping = false;
            self.last_start = start;
            self.last_end = end;

            // Ajuste progressivement la longueur au repos si l'utilisateur étire beaucoup le câble
            let target_len = (start.distance(end) * 1.15 + 30.0) / (CABLE_NODES - 1) as f32;
            self.rest_segment_len = self.rest_segment_len * 0.85 + target_len * 0.15;
        }

        if self.sleeping {
            self.pos[0] = start;
            self.pos[CABLE_NODES - 1] = end;
            return;
        }

        let dt_sq = dt * dt;

        // 1. Intégration de Verlet sur les nœuds libres (1 .. CABLE_NODES - 1)
        for i in 1..(CABLE_NODES - 1) {
            let vx = (self.pos[i].x - self.old_pos[i].x) * DAMPING;
            let vy = (self.pos[i].y - self.old_pos[i].y) * DAMPING;

            self.old_pos[i] = self.pos[i];
            self.pos[i].x += vx;
            self.pos[i].y += vy + GRAVITY * dt_sq;
        }

        // 2. Relaxation des contraintes de distance et ancrage des extrémités
        for _ in 0..CONSTRAINT_ITERATIONS {
            self.pos[0] = start;
            self.pos[CABLE_NODES - 1] = end;

            for i in 0..(CABLE_NODES - 1) {
                let dx = self.pos[i + 1].x - self.pos[i].x;
                let dy = self.pos[i + 1].y - self.pos[i].y;
                let dist = (dx * dx + dy * dy).sqrt().max(0.0001);
                let diff = (dist - self.rest_segment_len) / dist;

                let offset_x = dx * diff * 0.5;
                let offset_y = dy * diff * 0.5;

                if i > 0 {
                    self.pos[i].x += offset_x;
                    self.pos[i].y += offset_y;
                }
                if i + 1 < CABLE_NODES - 1 {
                    self.pos[i + 1].x -= offset_x;
                    self.pos[i + 1].y -= offset_y;
                }
            }
        }

        // 3. Détection de repos pour économiser le CPU
        let mut kinetic_energy = 0.0f32;
        for i in 1..(CABLE_NODES - 1) {
            let delta = Vec2::new(
                self.pos[i].x - self.old_pos[i].x,
                self.pos[i].y - self.old_pos[i].y,
            );
            kinetic_energy += delta.length_sq();
        }

        if kinetic_energy < SLEEP_ENERGY_THRESHOLD {
            self.sleeping = true;
        }
    }
}