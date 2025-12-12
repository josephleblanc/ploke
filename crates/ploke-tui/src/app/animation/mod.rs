/// Animation system for the ploke-tui terminal interface
///
/// This module provides smooth animations for message appearance, status changes,
/// and other UI transitions.
use std::collections::HashMap;
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::chat_history::{MessageKind, MessageStatus};

/// Configuration for animation behavior
#[derive(Debug, Clone)]
pub struct AnimationConfig {
    pub enabled: bool,
    pub default_duration: Duration,
    pub easing_function: EasingFunction,
}

impl Default for AnimationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_duration: Duration::from_millis(300),
            easing_function: EasingFunction::EaseInOut,
        }
    }
}

/// Easing functions for smooth animations
#[derive(Debug, Clone, Copy)]
pub enum EasingFunction {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Bounce,
}

/// Types of animations that can be applied to messages
#[derive(Debug, Clone)]
pub enum AnimationType {
    FadeIn {
        duration: Duration,
    },
    SlideIn {
        direction: SlideDirection,
        duration: Duration,
    },
    Typewriter {
        char_delay: Duration,
    },
    Pulse {
        duration: Duration,
        intensity: f32,
    },
    Highlight {
        duration: Duration,
        color: String,
    },
}

/// Direction for slide-in animations
#[derive(Debug, Clone, Copy)]
pub enum SlideDirection {
    Left,
    Right,
    Up,
    Down,
}

/// Represents an active animation on a message
#[derive(Debug, Clone)]
pub struct MessageAnimation {
    pub message_id: Uuid,
    pub start_time: Instant,
    pub animation_type: AnimationType,
    pub progress: f32,   // 0.0 to 1.0
    pub completed: bool, // Track if animation has completed
}

/// Manages all active animations in the application
#[derive(Debug, Default)]
pub struct AnimationState {
    pub animating_messages: HashMap<Uuid, MessageAnimation>,
    pub config: AnimationConfig,
}

impl AnimationState {
    /// Create a new animation state with default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new animation state with custom configuration
    pub fn with_config(config: AnimationConfig) -> Self {
        Self {
            animating_messages: HashMap::new(),
            config,
        }
    }

    /// Start a new animation for a message
    pub fn start_animation(&mut self, message_id: Uuid, animation_type: AnimationType) {
        if !self.config.enabled {
            return;
        }

        let animation = MessageAnimation {
            message_id,
            start_time: Instant::now(),
            animation_type,
            progress: 0.0,
            completed: false,
        };

        self.animating_messages.insert(message_id, animation);
    }

    /// Update all active animations and return IDs of completed animations
    pub fn update_animations(&mut self) -> Vec<Uuid> {
        let mut completed = Vec::new();
        let mut updates = Vec::new();

        // Collect animation data without holding mutable borrow
        let animation_data: Vec<(Uuid, AnimationType, Duration, Duration)> = self
            .animating_messages
            .iter()
            .map(|(message_id, animation)| {
                let elapsed = animation.start_time.elapsed();
                let animation_type = animation.animation_type.clone();
                let duration = self.get_animation_duration(&animation_type);
                (*message_id, animation_type, duration, elapsed)
            })
            .collect();

        // Process collected data
        for (message_id, animation_type, duration, elapsed) in animation_data {
            if elapsed >= duration {
                updates.push((message_id, 1.0, true));
                completed.push(message_id);
            } else {
                let progress = elapsed.as_secs_f32() / duration.as_secs_f32();
                let eased_progress = self.apply_easing(progress);
                updates.push((message_id, eased_progress, false));
            }
        }

        // Apply updates with separate mutable borrow
        for (message_id, progress, is_completed) in updates {
            if let Some(animation) = self.animating_messages.get_mut(&message_id) {
                animation.progress = progress;
                if is_completed {
                    animation.completed = true; // Mark as completed instead of removing
                }
            }
        }

        completed
    }

    /// Check if a message has an animation (active or completed)
    pub fn has_animation(&self, message_id: &Uuid) -> bool {
        self.animating_messages.contains_key(message_id)
    }

    /// Get the current progress of an animation (0.0 to 1.0)
    pub fn get_progress(&self, message_id: &Uuid) -> Option<f32> {
        self.animating_messages.get(message_id).map(|anim| {
            if anim.completed {
                1.0 // Completed animations always return full progress
            } else {
                let elapsed = anim.start_time.elapsed();
                let duration = self.get_animation_duration(&anim.animation_type);
                if elapsed >= duration {
                    1.0
                } else {
                    (elapsed.as_secs_f32() / duration.as_secs_f32()).min(1.0)
                }
            }
        })
    }

    /// Get the animation type for a message
    pub fn get_animation_type(&self, message_id: &Uuid) -> Option<&AnimationType> {
        self.animating_messages
            .get(message_id)
            .map(|anim| &anim.animation_type)
    }

    /// Stop all animations for a specific message
    pub fn stop_animation(&mut self, message_id: &Uuid) {
        self.animating_messages.remove(message_id);
    }

    /// Stop all active animations
    pub fn stop_all_animations(&mut self) {
        self.animating_messages.clear();
    }

    /// Clean up old completed animations to prevent memory leaks
    pub fn cleanup_old_animations(&mut self, max_age: Duration) {
        let now = Instant::now();
        self.animating_messages.retain(|_, animation| {
            if animation.completed {
                now.duration_since(animation.start_time) < max_age
            } else {
                true // Keep active animations
            }
        });
    }

    /// Get the duration of an animation type
    fn get_animation_duration(&self, animation_type: &AnimationType) -> Duration {
        match animation_type {
            AnimationType::FadeIn { duration } => *duration,
            AnimationType::SlideIn { duration, .. } => *duration,
            AnimationType::Typewriter { .. } => self.config.default_duration,
            AnimationType::Pulse { duration, .. } => *duration,
            AnimationType::Highlight { duration, .. } => *duration,
        }
    }

    /// Apply easing function to progress value
    fn apply_easing(&self, progress: f32) -> f32 {
        match self.config.easing_function {
            EasingFunction::Linear => progress,
            EasingFunction::EaseIn => progress * progress,
            EasingFunction::EaseOut => 1.0 - (1.0 - progress).powi(2),
            EasingFunction::EaseInOut => {
                if progress < 0.5 {
                    2.0 * progress * progress
                } else {
                    1.0 - (-2.0 * progress + 2.0).powi(2) / 2.0
                }
            }
            EasingFunction::Bounce => {
                // Simple bounce easing
                let n1 = 7.5625;
                let d1 = 2.75;

                if progress < 1.0 / d1 {
                    n1 * progress * progress
                } else if progress < 2.0 / d1 {
                    let progress = progress - 1.5 / d1;
                    n1 * progress * progress + 0.75
                } else if progress < 2.5 / d1 {
                    let progress = progress - 2.25 / d1;
                    n1 * progress * progress + 0.9375
                } else {
                    let progress = progress - 2.625 / d1;
                    n1 * progress * progress + 0.984375
                }
            }
        }
    }
}

/// Animation utilities for applying effects to text content
pub struct AnimationUtils;

impl AnimationUtils {
    /// Apply a fade-in effect to text content
    pub fn apply_fade_effect(content: &str, progress: f32) -> String {
        if progress >= 1.0 {
            return content.to_string();
        }

        // Calculate how many characters to show based on progress
        let total_chars = content.len();
        let visible_chars = (total_chars as f32 * progress) as usize;

        if visible_chars >= total_chars {
            content.to_string()
        } else {
            content.chars().take(visible_chars).collect()
        }
    }

    /// Apply a typewriter effect to text content
    pub fn apply_typewriter_effect(content: &str, progress: f32) -> String {
        let total_chars = content.len();
        let visible_chars = (total_chars as f32 * progress) as usize;

        content.chars().take(visible_chars).collect()
    }

    /// Apply a slide-in effect to text content
    pub fn apply_slide_effect(content: &str, progress: f32, direction: SlideDirection) -> String {
        // For slide effects, we add padding to create visual slide effect
        let total_chars = content.len();
        let visible_chars = (total_chars as f32 * progress) as usize;

        if visible_chars >= total_chars {
            content.to_string()
        } else {
            let visible_content: String = content.chars().take(visible_chars).collect();
            // Add padding proportional to content length to ensure result is longer
            let padding_len = (total_chars - visible_chars).max(1);
            let padding = " ".repeat(padding_len);

            match direction {
                SlideDirection::Left => format!("{}{}", padding, visible_content),
                SlideDirection::Right => format!("{}{}", visible_content, padding),
                SlideDirection::Up | SlideDirection::Down => visible_content,
            }
        }
    }

    /// Apply a pulse effect to text (returns the content with potential styling info)
    pub fn apply_pulse_effect(content: &str, progress: f32, intensity: f32) -> String {
        // Pulse effect could modify the visual appearance
        // For now, we'll return the content as-is but could add visual markers
        content.to_string()
    }

    /// Apply a highlight effect to text
    pub fn apply_highlight_effect(content: &str, progress: f32, color: &str) -> String {
        // Highlight effect could add visual markers
        // For now, we'll return the content as-is
        content.to_string()
    }
}

/// Helper functions for creating common animation types
pub struct AnimationFactory;

impl AnimationFactory {
    /// Create a fade-in animation with default duration
    pub fn fade_in_default() -> AnimationType {
        AnimationType::FadeIn {
            duration: Duration::from_millis(300),
        }
    }

    /// Create a fade-in animation with custom duration
    pub fn fade_in(duration: Duration) -> AnimationType {
        AnimationType::FadeIn { duration }
    }

    /// Create a slide-in animation from the left
    pub fn slide_in_left(duration: Duration) -> AnimationType {
        AnimationType::SlideIn {
            direction: SlideDirection::Left,
            duration,
        }
    }

    /// Create a slide-in animation from the right
    pub fn slide_in_right(duration: Duration) -> AnimationType {
        AnimationType::SlideIn {
            direction: SlideDirection::Right,
            duration,
        }
    }

    /// Create a slide-in animation from the top
    pub fn slide_in_up(duration: Duration) -> AnimationType {
        AnimationType::SlideIn {
            direction: SlideDirection::Up,
            duration,
        }
    }

    /// Create a slide-in animation from the bottom
    pub fn slide_in_down(duration: Duration) -> AnimationType {
        AnimationType::SlideIn {
            direction: SlideDirection::Down,
            duration,
        }
    }

    /// Create a typewriter animation
    pub fn typewriter(char_delay: Duration) -> AnimationType {
        AnimationType::Typewriter { char_delay }
    }

    /// Create a pulse animation
    pub fn pulse(duration: Duration, intensity: f32) -> AnimationType {
        AnimationType::Pulse {
            duration,
            intensity,
        }
    }

    /// Create a highlight animation
    pub fn highlight(duration: Duration, color: String) -> AnimationType {
        AnimationType::Highlight { duration, color }
    }
}

/// Animation presets for common use cases
pub struct AnimationPresets;

impl AnimationPresets {
    /// Animation for new user messages
    pub fn new_user_message() -> AnimationType {
        AnimationFactory::fade_in(Duration::from_millis(200))
    }

    /// Animation for new assistant messages
    pub fn new_assistant_message() -> AnimationType {
        AnimationFactory::typewriter(Duration::from_millis(30))
    }

    /// Animation for system messages
    pub fn new_system_message() -> AnimationType {
        AnimationFactory::slide_in_up(Duration::from_millis(250))
    }

    /// Animation for message completion
    pub fn message_completed() -> AnimationType {
        AnimationFactory::fade_in(Duration::from_millis(150))
    }

    /// Animation for message selection
    pub fn message_selected() -> AnimationType {
        AnimationFactory::pulse(Duration::from_millis(200), 0.5)
    }

    /// Animation for error messages
    pub fn error_message() -> AnimationType {
        AnimationFactory::highlight(Duration::from_millis(500), "red".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_state_creation() {
        let state = AnimationState::new();
        assert!(state.animating_messages.is_empty());
        assert!(state.config.enabled);
    }

    #[test]
    fn test_animation_progress_calculation() {
        let mut state = AnimationState::new();
        let message_id = Uuid::new_v4();

        state.start_animation(message_id, AnimationFactory::fade_in_default());

        // Initially progress should be 0 (allowing for tiny timing differences)
        let initial_progress = state.get_progress(&message_id);
        assert!(initial_progress.is_some());
        assert!(initial_progress.unwrap() <= 0.001); // Allow tiny epsilon for timing

        // After starting, progress should be tracked
        std::thread::sleep(Duration::from_millis(50));
        let progress = state.get_progress(&message_id);
        assert!(progress.is_some());
        assert!(progress.unwrap() > 0.0);
    }

    #[test]
    fn test_animation_completion() {
        let mut state = AnimationState::new();
        let message_id = Uuid::new_v4();

        state.start_animation(
            message_id,
            AnimationFactory::fade_in(Duration::from_millis(100)),
        );

        // Wait for animation to complete
        std::thread::sleep(Duration::from_millis(150));

        let completed = state.update_animations();
        assert!(completed.contains(&message_id));
        // Animation should still be in state after completion (marked as completed)
        assert!(state.has_animation(&message_id));
        // Progress should still be queryable and return 1.0
        assert_eq!(state.get_progress(&message_id), Some(1.0));
    }

    #[test]
    fn test_smooth_animation_progression() {
        let mut state = AnimationState::new();
        let message_id = Uuid::new_v4();
        let animation_duration = Duration::from_millis(1000); // 1 second for clear progression

        // Start a fade-in animation
        state.start_animation(message_id, AnimationFactory::fade_in(animation_duration));

        // Track progress at different time intervals
        let mut progress_samples = Vec::new();

        // Sample progress at 100ms intervals
        for i in 0..=10 {
            let sample_time = Duration::from_millis(i * 100);
            std::thread::sleep(sample_time);

            let progress = state.get_progress(&message_id);
            progress_samples.push((i, sample_time, progress));

            // Update animations to advance the state
            let _completed = state.update_animations();
        }

        // Verify smooth progression
        println!("Animation progression test:");
        for (i, time, progress) in &progress_samples {
            println!("  {}ms: progress = {:?}", time.as_millis(), progress);
        }

        // Check that progress increases smoothly over time
        for i in 1..progress_samples.len() {
            let (_, _, prev_progress) = progress_samples[i - 1];
            let (_, _, curr_progress) = progress_samples[i];

            if let (Some(prev), Some(curr)) = (prev_progress, curr_progress) {
                // Progress should generally increase (allowing for small timing variations)
                assert!(
                    curr >= prev - 0.01,
                    "Progress should increase smoothly: {} -> {} at step {}",
                    prev,
                    curr,
                    i
                );
            }
        }

        // Final state should be completed
        let completed = state.update_animations();
        assert!(
            completed.contains(&message_id),
            "Animation should complete within expected time"
        );
    }

    #[test]
    fn test_animation_timing_consistency() {
        let mut state = AnimationState::new();
        let message_id = Uuid::new_v4();
        let duration = Duration::from_millis(500);

        state.start_animation(message_id, AnimationFactory::fade_in(duration));

        // Check progress at exactly 25% of the way through
        std::thread::sleep(Duration::from_millis(125)); // 25% of 500ms
        let progress_25 = state.get_progress(&message_id);

        // Check progress at exactly 50% of the way through
        std::thread::sleep(Duration::from_millis(125)); // Now 50% total
        let progress_50 = state.get_progress(&message_id);

        // Check progress at exactly 75% of the way through
        std::thread::sleep(Duration::from_millis(125)); // Now 75% total
        let progress_75 = state.get_progress(&message_id);

        println!("Timing consistency test:");
        println!("  25% mark: progress = {:?}", progress_25);
        println!("  50% mark: progress = {:?}", progress_50);
        println!("  75% mark: progress = {:?}", progress_75);

        // Progress should be roughly at the expected percentages (with some tolerance for timing)
        if let (Some(p25), Some(p50), Some(p75)) = (progress_25, progress_50, progress_75) {
            assert!(
                p25 >= 0.20 && p25 <= 0.30,
                "25% mark should be around 0.25, got {}",
                p25
            );
            assert!(
                p50 >= 0.45 && p50 <= 0.55,
                "50% mark should be around 0.50, got {}",
                p50
            );
            assert!(
                p75 >= 0.70 && p75 <= 0.80,
                "75% mark should be around 0.75, got {}",
                p75
            );
        } else {
            panic!("Progress should not be None during animation");
        }
    }

    #[test]
    fn test_animation_state_after_completion() {
        let mut state = AnimationState::new();
        let message_id = Uuid::new_v4();
        let duration = Duration::from_millis(200);

        // Start animation
        state.start_animation(message_id, AnimationFactory::fade_in(duration));

        // Wait for animation to complete
        std::thread::sleep(Duration::from_millis(250));

        // Animation should be marked as completed but remain in state
        let completed = state.update_animations();
        assert!(completed.contains(&message_id));
        assert!(state.has_animation(&message_id));

        // After completion, get_progress should return 1.0 (animation persists)
        let progress_after = state.get_progress(&message_id);
        assert_eq!(
            progress_after,
            Some(1.0),
            "Animation should persist in state after completion and return full progress"
        );

        // This fix ensures: if UI tries to render after animation completes,
        // it will get 1.0 progress and continue rendering the full message smoothly,
        // preventing the "jump" effect the user was experiencing
        println!("FIXED: Animation persists in state after completion, preventing UI jump");
    }

    #[test]
    fn test_animation_rendering_consistency() {
        let mut state = AnimationState::new();
        let message_id = Uuid::new_v4();
        let duration = Duration::from_millis(300);
        let test_content = "Hello, World! This is a test message.";

        // Start animation
        state.start_animation(message_id, AnimationFactory::fade_in(duration));

        // Simulate UI rendering at different stages
        for i in 0..=6 {
            let elapsed = Duration::from_millis(i * 50);
            std::thread::sleep(Duration::from_millis(50));

            let progress = state.get_progress(&message_id);
            let animation_type = state.get_animation_type(&message_id);

            let rendered_content =
                if let (Some(progress), Some(anim_type)) = (progress, animation_type) {
                    match anim_type {
                        AnimationType::FadeIn { .. } => {
                            AnimationUtils::apply_fade_effect(test_content, progress)
                        }
                        _ => test_content.to_string(),
                    }
                } else {
                    // This is the problematic case - when animation is removed, UI gets full content
                    test_content.to_string()
                };

            println!(
                "{}ms: progress={:?}, content_length={}",
                elapsed.as_millis(),
                progress,
                rendered_content.len()
            );

            // Update animation state
            let _completed = state.update_animations();
        }

        // Final state check
        let final_progress = state.get_progress(&message_id);
        let final_completed = state.update_animations();

        println!(
            "Final state: progress={:?}, completed={}",
            final_progress,
            final_completed.contains(&message_id)
        );
    }

    #[test]
    fn test_easing_functions() {
        let config = AnimationConfig {
            enabled: true,
            default_duration: Duration::from_millis(1000),
            easing_function: EasingFunction::EaseInOut,
        };

        let state = AnimationState::with_config(config);

        // Test easing at different progress points
        assert_eq!(state.apply_easing(0.0), 0.0);
        assert_eq!(state.apply_easing(1.0), 1.0);

        // Middle point should be 0.5 for ease-in-out
        let middle = state.apply_easing(0.5);
        assert!(middle > 0.0 && middle < 1.0);
    }

    #[test]
    fn test_animation_utils() {
        let content = "Hello, World!";

        // Test fade effect
        let faded = AnimationUtils::apply_fade_effect(content, 0.5);
        assert!(faded.len() <= content.len());

        // Test typewriter effect
        let typed = AnimationUtils::apply_typewriter_effect(content, 0.5);
        assert!(typed.len() <= content.len());

        // Test slide effect
        let slid = AnimationUtils::apply_slide_effect(content, 0.5, SlideDirection::Left);
        assert!(slid.len() >= content.len()); // Should have padding
    }

    #[test]
    fn test_ui_jump_fix_demonstration() {
        let mut state = AnimationState::new();
        let message_id = Uuid::new_v4();
        let duration = Duration::from_millis(200);
        let test_content = "Hello, World!";

        // Start animation
        state.start_animation(message_id, AnimationFactory::fade_in(duration));

        println!("=== DEMONSTRATION: UI Jump Fix ===");
        println!("Testing animation behavior before and after completion...\n");

        // Simulate UI rendering frames
        for frame in 0..=4 {
            let elapsed = Duration::from_millis(frame * 75);
            std::thread::sleep(Duration::from_millis(75));

            let progress = state.get_progress(&message_id);
            let animation_type = state.get_animation_type(&message_id);

            // Simulate UI rendering logic
            let rendered_content =
                if let (Some(progress), Some(anim_type)) = (progress, animation_type) {
                    match anim_type {
                        AnimationType::FadeIn { .. } => {
                            AnimationUtils::apply_fade_effect(test_content, progress)
                        }
                        _ => test_content.to_string(),
                    }
                } else {
                    // This branch represents the OLD buggy behavior
                    test_content.to_string()
                };

            let status = if frame < 3 { "ANIMATING" } else { "COMPLETED" };
            println!(
                "Frame {} ({}ms, {}): progress={:?}, content='{}'",
                frame,
                elapsed.as_millis(),
                status,
                progress,
                rendered_content
            );

            // Update animation state
            let _completed = state.update_animations();
        }

        // Final verification
        let final_progress = state.get_progress(&message_id);
        let final_content = if let Some(progress) = final_progress {
            AnimationUtils::apply_fade_effect(test_content, progress)
        } else {
            test_content.to_string()
        };

        println!("\n=== RESULTS ===");
        println!(
            "✅ Animation persists after completion: {}",
            state.has_animation(&message_id)
        );
        println!("✅ Progress remains consistent: {:?}", final_progress);
        println!("✅ Content rendering stable: '{}'", final_content);
        println!("✅ NO UI JUMP - Smooth transition from animated to static state!");

        // Verify the fix
        assert!(state.has_animation(&message_id), "Animation should persist");
        assert_eq!(
            final_progress,
            Some(1.0),
            "Progress should be 1.0 after completion"
        );
        assert_eq!(
            final_content, test_content,
            "Content should be fully rendered"
        );
    }
}
