//! Instrument submodule.
use super::*;

const MELODY_HIT_LINE: f32 = 0.78;
const MELODY_SPAWN_Y: f32 = 0.04;

const MELODY_TWINKLE: &[MelodyNote] = &[
    (60, 0, 1),
    (60, 400, 1),
    (67, 400, 3),
    (67, 400, 3),
    (69, 400, 4),
    (69, 400, 4),
    (67, 520, 3),
    (65, 500, 2),
    (65, 400, 2),
    (64, 400, 1),
    (64, 400, 1),
    (62, 400, 0),
    (62, 400, 0),
    (60, 520, 1),
    // echo phrase with a soft chord
    (60, 560, 1),
    (64, 0, 2),
    (67, 0, 3),
];

const MELODY_JOY: &[MelodyNote] = &[
    (64, 0, 2),
    (64, 340, 2),
    (65, 340, 3),
    (67, 340, 4),
    (67, 340, 4),
    (65, 340, 3),
    (64, 340, 2),
    (62, 340, 1),
    (60, 340, 0),
    (60, 340, 0),
    (62, 340, 1),
    (64, 340, 2),
    (64, 460, 2),
    (62, 220, 1),
    (62, 460, 1),
    (64, 500, 2),
    (64, 340, 2),
    (65, 340, 3),
    (67, 340, 4),
    (67, 340, 4),
    (65, 340, 3),
    (64, 340, 2),
    (62, 340, 1),
    (60, 340, 0),
    (60, 340, 0),
    (62, 340, 1),
    (64, 340, 2),
    (62, 460, 1),
    (60, 220, 0),
    (60, 520, 0),
];

const MELODY_YOLKA: &[MelodyNote] = &[
    (60, 0, 1),
    (62, 360, 2),
    (64, 360, 3),
    (60, 360, 1),
    (64, 400, 3),
    (60, 360, 1),
    (64, 400, 3),
    (62, 460, 2),
    (64, 360, 3),
    (65, 360, 4),
    (65, 360, 4),
    (64, 360, 3),
    (62, 360, 2),
    (65, 460, 4),
    (64, 460, 3),
    (65, 360, 4),
    (67, 360, 4),
    (64, 360, 3),
    (67, 400, 4),
    (64, 360, 3),
    (67, 400, 4),
    (65, 460, 4),
    (67, 360, 4),
    (69, 360, 4),
    (69, 360, 4),
    (67, 360, 4),
    (65, 360, 3),
    (64, 520, 2),
];

const MELODY_TETRIS: &[MelodyNote] = &[
    (76, 0, 4),
    (71, 300, 2),
    (72, 300, 3),
    (74, 300, 4),
    (72, 300, 3),
    (71, 300, 2),
    (69, 300, 1),
    (69, 300, 1),
    (72, 300, 3),
    (76, 300, 4),
    (74, 300, 4),
    (72, 300, 3),
    (71, 300, 2),
    (71, 300, 2),
    (72, 300, 3),
    (74, 300, 4),
    (76, 300, 4),
    (72, 300, 3),
    (69, 300, 1),
    (69, 460, 1),
    (74, 380, 4),
    (77, 300, 4),
    (81, 300, 4),
    (79, 300, 4),
    (77, 300, 4),
    (76, 300, 4),
    (72, 300, 3),
    (76, 300, 4),
    (74, 300, 4),
    (72, 300, 3),
    (71, 300, 2),
    (71, 300, 2),
    (72, 300, 3),
    (74, 300, 4),
    (76, 300, 4),
    (72, 300, 3),
    (69, 300, 1),
    (69, 500, 1),
];

/// Happy Birthday (simplified, C major).
const MELODY_BIRTHDAY: &[MelodyNote] = &[
    (60, 0, 1),
    (60, 280, 1),
    (62, 420, 2),
    (60, 420, 1),
    (65, 420, 3),
    (64, 560, 2),
    (60, 500, 1),
    (60, 280, 1),
    (62, 420, 2),
    (60, 420, 1),
    (67, 420, 4),
    (65, 560, 3),
    (60, 500, 1),
    (60, 280, 1),
    (72, 420, 4),
    (69, 420, 4),
    (65, 420, 3),
    (64, 420, 2),
    (62, 560, 1),
    (70, 500, 4),
    (70, 280, 4),
    (69, 420, 4),
    (65, 420, 3),
    (67, 420, 4),
    (65, 600, 3),
];

/// Jingle Bells (simplified).
const MELODY_JINGLE: &[MelodyNote] = &[
    (64, 0, 2),
    (64, 300, 2),
    (64, 300, 2),
    (64, 500, 2),
    (64, 300, 2),
    (64, 300, 2),
    (64, 500, 2),
    (64, 300, 2),
    (67, 300, 4),
    (60, 300, 1),
    (62, 300, 2),
    (64, 560, 2),
    (65, 500, 3),
    (65, 300, 3),
    (65, 300, 3),
    (65, 300, 3),
    (65, 300, 3),
    (64, 300, 2),
    (64, 300, 2),
    (64, 300, 2),
    (64, 220, 2),
    (62, 300, 1),
    (62, 300, 1),
    (64, 300, 2),
    (62, 500, 1),
    (67, 560, 4),
];

fn melody_song(song: u8) -> &'static [MelodyNote] {
    match song {
        1 => MELODY_JOY,
        2 => MELODY_YOLKA,
        3 => MELODY_TETRIS,
        4 => MELODY_BIRTHDAY,
        5 => MELODY_JINGLE,
        _ => MELODY_TWINKLE,
    }
}

fn melody_song_title(song: u8) -> &'static str {
    match song {
        1 => "ОДА",
        2 => "ELKA",
        3 => "TETRIS",
        4 => "BDAY",
        5 => "JINGLE",
        _ => "STAR",
    }
}

fn melody_song_title_ru(song: u8) -> &'static str {
    match song {
        1 => "Ода к радости",
        2 => "Ёлочка",
        3 => "Тетрис",
        4 => "День рождения",
        5 => "Jingle Bells",
        _ => "Звёздочка",
    }
}

fn melody_ball_color(note: u8) -> [u8; 3] {
    match note % 12 {
        0 => [251, 113, 133],
        1 => [251, 146, 60],
        2 => [251, 191, 36],
        3 => [163, 230, 53],
        4 => [52, 211, 153],
        5 => [45, 212, 191],
        6 => [34, 211, 238],
        7 => [96, 165, 250],
        8 => [129, 140, 248],
        9 => [167, 139, 250],
        10 => [232, 121, 249],
        _ => [244, 114, 182],
    }
}

fn melody_note_name(note: u8) -> &'static str {
    match note % 12 {
        0 => "C",
        1 => "C#",
        2 => "D",
        3 => "D#",
        4 => "E",
        5 => "F",
        6 => "F#",
        7 => "G",
        8 => "G#",
        9 => "A",
        10 => "A#",
        _ => "B",
    }
}

fn melody_lane_x(lane: u8) -> f32 {
    // Full width — five rooms with air between them.
    0.10 + f32::from(lane.clamp(0, 4)) * 0.20
}

/// 0 Ладонь · 1 Кулак · 2 Мастер
fn melody_needs_fist(diff: u8) -> bool {
    diff >= 1
}

fn melody_mode_tag(diff: u8) -> &'static str {
    // Bitmap HUD font is Latin-only.
    match diff {
        0 => "PALM",
        1 => "FIST",
        _ => "MASTER",
    }
}

fn melody_mode_hint_ru(diff: u8) -> &'static str {
    match diff {
        0 => "ладонью лови",
        1 => "наведи и сожми кулак",
        _ => "кулак · точнее",
    }
}

/// look_ahead_ms, note_r, palm_r, perfect, good, gap_scale, max_lives, max_on_screen
fn melody_diff_params(diff: u8) -> (u64, f32, f32, f32, f32, f32, u8, usize) {
    match diff {
        // Ладонь — просторно, мало нот на экране
        0 => (2100, 0.052, 0.072, 0.09, 0.18, 1.40, 5, 3),
        // Кулак
        1 => (1800, 0.048, 0.066, 0.08, 0.15, 1.28, 4, 3),
        // Мастер
        _ => (1400, 0.042, 0.058, 0.055, 0.11, 1.10, 3, 4),
    }
}

fn melody_note_hit_at(song: &[MelodyNote], index: usize, origin: u64, gap_scale: f32) -> u64 {
    let mut t = origin;
    let mut prev_lane: Option<u8> = song.first().map(|n| n.2);
    for i in 1..=index {
        let Some(&( _note, gap, lane)) = song.get(i) else {
            t = t.saturating_add(400);
            continue;
        };
        // gap == 0 → chord: same beat, different lanes (keep together).
        if gap == 0 {
            prev_lane = Some(lane);
            continue;
        }
        let mut spaced = ((gap as f32) * gap_scale).max(360.0);
        // Same-lane repeats need more air so notes don't stack visually.
        if prev_lane == Some(lane) {
            spaced = spaced.max(560.0);
        }
        t = t.saturating_add(spaced as u64);
        prev_lane = Some(lane);
    }
    t
}

fn melody_judge_at(y: f32, perfect: f32, good: f32) -> MelodyJudge {
    let d = (y - MELODY_HIT_LINE).abs();
    if d <= perfect {
        MelodyJudge::Perfect
    } else if d <= good {
        MelodyJudge::Good
    } else {
        MelodyJudge::Ok
    }
}


impl Stage {
    pub(crate) fn reset_melody(&mut self, now: u64) {
        let (_, _, _, _, _, _, max_lives, _) = melody_diff_params(self.music.melody_difficulty);
        self.music.melody_balls.clear();
        self.music.melody_index = 0;
        self.music.melody_score = 0;
        self.music.melody_combo = 0;
        self.music.melody_best_combo = 0;
        self.music.melody_perfects = 0;
        self.music.melody_goods = 0;
        self.music.melody_oks = 0;
        self.music.melody_misses = 0;
        self.music.melody_max_lives = max_lives;
        self.music.melody_lives = max_lives;
        self.music.melody_origin = if now == 0 { 0 } else { now.saturating_add(900) };
        self.music.melody_next_at = self.music.melody_origin;
        self.music.melody_tips.clear();
        self.music.melody_aim_lock = None;
        self.music.melody_flash = None;
        self.music.melody_banner_until = 0;
        self.label = format!(
            "Мелодия · {} · {}",
            melody_song_title_ru(self.music.melody_song),
            melody_mode_hint_ru(self.music.melody_difficulty)
        );
    }

    // --- ПИАНИНО ---
    pub(crate) fn play_melody(&mut self, now: u64, hands: &[TrackedHand]) {
        let song = melody_song(self.music.melody_song);
        if song.is_empty() {
            return;
        }
        let diff = self.music.melody_difficulty;
        let needs_fist = melody_needs_fist(diff);
        let (look_ahead, note_r, palm_r, perfect, good, gap_scale, max_lives, max_on_screen) =
            melody_diff_params(diff);
        self.music.melody_max_lives = max_lives;
        if self.music.melody_lives == 0 || self.music.melody_lives > max_lives {
            self.music.melody_lives = max_lives;
        }

        if self.music.melody_origin == 0 {
            self.music.melody_origin = now.saturating_add(900);
        }

        self.music.melody_tips.clear();
        self.music.melody_aim_lock = None;
        for hand in hands {
            self.music.melody_tips.push(palm_of(hand));
        }

        while self.music.melody_balls.len() < max_on_screen
            && self.music.melody_index < song.len()
        {
            let hit_at = melody_note_hit_at(
                song,
                self.music.melody_index,
                self.music.melody_origin,
                gap_scale,
            );
            if now.saturating_add(look_ahead) < hit_at {
                break;
            }
            let (note, _gap, lane) = song[self.music.melody_index];
            let lane = lane.clamp(0, 4);
            self.music.melody_balls.push(MelodyBall {
                x: melody_lane_x(lane),
                y: MELODY_SPAWN_Y,
                note,
                radius: note_r,
                born_at: now,
                hit_at,
                color: melody_ball_color(note),
                lane,
            });
            self.music.melody_index += 1;
        }

        let travel = (MELODY_HIT_LINE - MELODY_SPAWN_Y).max(0.2);
        let mut missed = 0u32;
        let mut write = 0usize;
        let len = self.music.melody_balls.len();
        for read in 0..len {
            let hit_at = self.music.melody_balls[read].hit_at;
            let y = if now >= hit_at {
                MELODY_HIT_LINE
                    + ((now - hit_at) as f32 / look_ahead as f32) * travel * 0.85
            } else {
                let remain = hit_at.saturating_sub(now) as f32;
                MELODY_HIT_LINE - (remain / look_ahead as f32) * travel
            };
            self.music.melody_balls[read].y = y.clamp(0.02, 0.98);
            if y > MELODY_HIT_LINE + good + 0.08 {
                missed = missed.saturating_add(1);
                continue;
            }
            if write != read {
                self.music.melody_balls.swap(write, read);
            }
            write += 1;
        }
        self.music.melody_balls.truncate(write);

        if missed > 0 {
            self.music.melody_misses = self.music.melody_misses.saturating_add(missed);
            self.music.melody_combo = 0;
            let lost = missed.min(u32::from(self.music.melody_lives)) as u8;
            self.music.melody_lives = self.music.melody_lives.saturating_sub(lost);
            if self.music.melody_lives == 0 {
                let title = melody_song_title_ru(self.music.melody_song);
                self.music.melody_balls.clear();
                self.music.melody_index = 0;
                self.music.melody_score = 0;
                self.music.melody_perfects = 0;
                self.music.melody_goods = 0;
                self.music.melody_oks = 0;
                self.music.melody_aim_lock = None;
                self.music.melody_flash = None;
                self.music.melody_lives = max_lives;
                self.music.melody_origin = now.saturating_add(1200);
                self.label = format!("Жизни кончились · «{}» сначала", title);
                self.last_hit = now;
                return;
            }
            self.label = format!(
                "Мимо · жизни {}/{}",
                self.music.melody_lives, max_lives
            );
            self.last_hit = now;
        }

        let palm_reach = palm_r * 1.12 * self.sensitivity.clamp(0.75, 1.5);
        let mut claimed = vec![false; self.music.melody_balls.len()];
        let mut hits: Vec<(usize, MelodyJudge)> = Vec::with_capacity(4);

        for hand in hands {
            let palm = palm_of(hand);
            let mut best: Option<(usize, f32)> = None;
            for (ball_i, ball) in self.music.melody_balls.iter().enumerate() {
                if claimed[ball_i] {
                    continue;
                }
                if ball.y < MELODY_HIT_LINE - good - 0.10 {
                    continue;
                }
                let need = palm_reach + ball.radius;
                let d = dist2(palm, (ball.x, ball.y));
                if d <= need && best.map(|(_, bd)| d < bd).unwrap_or(true) {
                    best = Some((ball_i, d));
                }
            }

            let Some((ball_i, _)) = best else {
                continue;
            };
            let ball = &self.music.melody_balls[ball_i];
            self.music.melody_aim_lock = Some((ball.x, ball.y));

            let should_hit = if needs_fist {
                let pose = pose_of(hand, 0.45);
                let was_fist = self
                    .prev_for(hand)
                    .is_some_and(|sample| sample.pose == Pose::Fist);
                pose == Pose::Fist && !was_fist
            } else {
                true
            };
            if !should_hit {
                continue;
            }

            claimed[ball_i] = true;
            hits.push((
                ball_i,
                melody_judge_at(ball.y, perfect, good),
            ));
        }

        hits.sort_by_key(|(i, _)| std::cmp::Reverse(*i));
        let fist_bonus = if needs_fist { 1u32 } else { 0 };
        for (ball_i, judge) in hits {
            let Some(ball) = self.music.melody_balls.get(ball_i).cloned() else {
                continue;
            };
            self.music.melody_balls.remove(ball_i);

            let (points, vel_mul, judge_tag, flash_kind) = match judge {
                MelodyJudge::Perfect => {
                    self.music.melody_perfects = self.music.melody_perfects.saturating_add(1);
                    (5u32 + fist_bonus, 1.0f32, "PERFECT", 2u8)
                }
                MelodyJudge::Good => {
                    self.music.melody_goods = self.music.melody_goods.saturating_add(1);
                    (3 + fist_bonus, 0.88, "GOOD", 1)
                }
                MelodyJudge::Ok => {
                    self.music.melody_oks = self.music.melody_oks.saturating_add(1);
                    (1 + fist_bonus, 0.72, "OK", 0)
                }
            };
            self.music.melody_combo = self.music.melody_combo.saturating_add(1);
            self.music.melody_best_combo = self
                .music
                .melody_best_combo
                .max(self.music.melody_combo);
            let combo_bonus = (self.music.melody_combo / 3).min(6);
            self.music.melody_score = self
                .music
                .melody_score
                .saturating_add(points + combo_bonus);

            let vel = self.velocity(
                ((0.58 + vel_mul * 0.38) + self.music.melody_combo as f32 * 0.02).min(1.0),
            );
            let life = match judge {
                MelodyJudge::Perfect => 520,
                MelodyJudge::Good => 400,
                MelodyJudge::Ok => 320,
            };
            self.shot(0, ball.note, vel, now + life);
            self.music.lit_key = Some((ball.note, now + 180));
            self.music.melody_flash = Some((ball.x, ball.y, now + 160, flash_kind));
            self.music.melody_aim_lock = None;
            self.label = format!(
                "{} · {} · x{}",
                judge_tag,
                melody_note_name(ball.note),
                self.music.melody_combo
            );
            self.last_hit = now;
        }

        if let Some((_, _, until, _)) = self.music.melody_flash {
            if now > until {
                self.music.melody_flash = None;
            }
        }

        if self.music.melody_index >= song.len() && self.music.melody_balls.is_empty() {
            self.music.melody_index = 0;
            self.music.melody_origin = now.saturating_add(1000);
            if self.music.melody_lives < max_lives {
                self.music.melody_lives += 1;
            }
            self.label = format!(
                "Круг · {} · x{}",
                self.music.melody_score,
                self.music.melody_best_combo
            );
        }
    }

    pub(crate) fn paint_melody(&self, image: &mut RgbImage) {
        let diff = self.music.melody_difficulty;
        let needs_fist = melody_needs_fist(diff);
        let (_look, _note_r, palm_r, _perfect, _good, _gap, max_lives, _) =
            melody_diff_params(diff);
        let armed = self.music.melody_aim_lock.is_some();

        // Overlay only — never wash out the camera.
        thick_hline(image, 0.06, 0.94, MELODY_HIT_LINE, [255, 255, 255], 0.85, 3);
        for lane in 0..5u8 {
            let x = melody_lane_x(lane);
            // Soft lane guides over the live video.
            vline(image, x, 0.08, MELODY_HIT_LINE - 0.02, [255, 255, 255], 0.14);
            thick_hline(image, x - 0.016, x + 0.016, MELODY_HIT_LINE, [255, 255, 255], 0.7, 3);
        }

        let lock = self.music.melody_aim_lock;
        for ball in &self.music.melody_balls {
            let (cx, cy) = px(image, ball.x, ball.y);
            let radius = (ball.radius * image.width() as f32).round().max(10.0) as i32;
            let focused = lock.is_some_and(|(lx, ly)| {
                (lx - ball.x).abs() < 0.001 && (ly - ball.y).abs() < 0.001
            });
            // Opaque notes with a white rim so they read on any camera background.
            disc(image, cx, cy, radius + 2, [255, 255, 255], 1.0);
            disc(image, cx, cy, radius + if focused { 1 } else { 0 }, ball.color, 1.0);
            disc(image, cx, cy, (radius / 3).max(2), [255, 255, 255], 1.0);
            if focused {
                ring(
                    image,
                    cx,
                    cy,
                    radius as f32 + 7.0,
                    if needs_fist {
                        [251, 146, 60]
                    } else {
                        [125, 211, 252]
                    },
                    0.9,
                );
            }
        }

        // Palm orb — ring + small core (few pixels), not a big translucent disc.
        let orb_px = (palm_r * image.width() as f32).round().max(10.0) as i32;
        for palm in &self.music.melody_tips {
            let (cx, cy) = px(image, palm.0, palm.1);
            let ring_color = if needs_fist && armed {
                [251, 146, 60]
            } else if needs_fist {
                [203, 213, 225]
            } else {
                [125, 211, 252]
            };
            ring(image, cx, cy, orb_px as f32, ring_color, 0.75);
            ring(image, cx, cy, orb_px as f32 * 0.55, ring_color, 0.35);
            disc(image, cx, cy, 4, [248, 250, 252], 1.0);
        }

        if let Some((x, y, _, kind)) = self.music.melody_flash {
            let (cx, cy) = px(image, x, y);
            let color = match kind {
                2 => [52, 211, 153],
                1 => [250, 204, 21],
                _ => [148, 163, 184],
            };
            disc(image, cx, cy, 6, color, 1.0);
        }

        draw_label(
            image,
            0.06,
            0.025,
            melody_mode_tag(diff),
            if needs_fist {
                [251, 146, 60]
            } else {
                [125, 211, 252]
            },
            0.95,
            1,
        );
        draw_label(
            image,
            0.20,
            0.025,
            melody_song_title(self.music.melody_song),
            [248, 250, 252],
            0.9,
            1,
        );
        let song_len = melody_song(self.music.melody_song).len().max(1) as f32;
        let progress = (self.music.melody_index as f32 / song_len).clamp(0.0, 1.0);
        thick_hline(image, 0.06, 0.56, 0.055, [51, 65, 85], 0.55, 2);
        if progress > 0.0 {
            thick_hline(
                image,
                0.06,
                0.06 + 0.50 * progress,
                0.055,
                if needs_fist {
                    [251, 146, 60]
                } else {
                    [56, 189, 248]
                },
                0.9,
                2,
            );
        }
        for i in 0..max_lives {
            let lit = i < self.music.melody_lives;
            let lx = 0.78 + f32::from(i) * 0.032;
            let (cx, cy) = px(image, lx, 0.035);
            disc(
                image,
                cx,
                cy,
                4,
                if lit {
                    [244, 63, 94]
                } else {
                    [71, 85, 105]
                },
                1.0,
            );
        }
        if self.music.melody_score > 0 {
            draw_label(
                image,
                0.06,
                0.07,
                &format!("{}", self.music.melody_score),
                [226, 232, 240],
                0.85,
                1,
            );
        }
        if needs_fist && !self.music.melody_tips.is_empty() {
            draw_label(
                image,
                0.06,
                0.93,
                if armed { "FIST NOW" } else { "AIM" },
                [226, 232, 240],
                0.7,
                1,
            );
        }
    }

    // =========================================================================
    // --- ПРОФЕССИОНАЛЬНАЯ ГИТАРА: РИТМ, АККОРДЫ, СОЛО-ГРИФ И БОЙ ---
    // =========================================================================

}
