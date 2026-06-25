// SPS Emotional Companion — Emotion Manager
// Phase 14: Manages emotion state machine + transitions

using System;
using System.Collections;
using UnityEngine;

namespace SPS.Companion
{
    /// <summary>
    /// Manages emotion state transitions with context-aware blending.
    /// Works with CompanionController to drive 3D facial expressions.
    /// </summary>
    public class EmotionManager : MonoBehaviour
    {
        [Header("References")]
        public CompanionController Companion;

        [Header("Emotion Settings")]
        public float BaseEmotionDuration = 5f;
        public float ConversationEmotionDuration = 10f;
        public bool AutoReturnToNeutral = true;

        private CompanionEmotion _previousEmotion = CompanionEmotion.Neutral;
        private CompanionEmotion _lockedEmotion = CompanionEmotion.Neutral;
        private float _emotionLockTimer = 0f;
        private bool _emotionLocked = false;

        void Update()
        {
            if (_emotionLocked)
            {
                _emotionLockTimer -= Time.deltaTime;
                if (_emotionLockTimer <= 0f)
                {
                    _emotionLocked = false;
                    if (AutoReturnToNeutral)
                    {
                        SetEmotion(_previousEmotion);
                    }
                }
            }
        }

        /// <summary>
        /// Set an emotion that will auto-return after a duration.
        /// </summary>
        public void SetEmotion(CompanionEmotion emotion, float duration = 0f)
        {
            if (duration <= 0f) duration = BaseEmotionDuration;

            _previousEmotion = emotion;
            _lockedEmotion = emotion;
            _emotionLockTimer = duration;
            _emotionLocked = true;

            if (Companion != null)
                Companion.SetEmotion(emotion);
        }

        /// <summary>
        /// Lock an emotion indefinitely (e.g. during conversation).
        /// </summary>
        public void LockEmotion(CompanionEmotion emotion)
        {
            _emotionLocked = false;
            _lockedEmotion = emotion;
            if (Companion != null)
                Companion.SetEmotion(emotion);
        }

        /// <summary>
        /// Release a locked emotion and return to neutral.
        /// </summary>
        public void ReleaseEmotion()
        {
            _emotionLocked = false;
            if (Companion != null)
                Companion.SetEmotion(CompanionEmotion.Neutral);
        }

        /// <summary>
        /// React to a user message with an appropriate emotion.
        /// </summary>
        public void ReactToMessage(string message)
        {
            string lower = message.ToLower();

            if (lower.Contains("سعيد") || lower.Contains("happy") || lower.Contains("ممتاز") || lower.Contains("شكرا"))
                SetEmotion(CompanionEmotion.Happy);
            else if (lower.Contains("حزين") || lower.Contains("sad") || lower.Contains("مشكلة") || lower.Contains("خطأ"))
                SetEmotion(CompanionEmotion.Sad);
            else if (lower.Contains("غاضب") || lower.Contains("angry") || lower.Contains("محبط"))
                SetEmotion(CompanionEmotion.Angry);
            else if (lower.Contains("مفاجأة") || lower.Contains("wow") || lower.Contains("ماذا"))
                SetEmotion(CompanionEmotion.Surprised);
            else
                SetEmotion(CompanionEmotion.Thinking, 2f);
        }

        /// <summary>
        /// Set listening mode (slight smile, attentive).
        /// </summary>
        public void SetListening()
        {
            LockEmotion(CompanionEmotion.Listening);
        }

        /// <summary>
        /// Set thinking mode (during LLM processing).
        /// </summary>
        public void SetThinking()
        {
            LockEmotion(CompanionEmotion.Thinking);
        }
    }
}
