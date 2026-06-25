// SPS Emotional Companion — Unity 3D Controller
// Phase 14: 3D Face Tracking + Lip Sync + Emotional Interaction
//
// This script is the main controller for the Unity-based emotional
// companion. It:
// 1. Connects to the SPS kernel via HTTP
// 2. Receives text/emotion commands from SPS
// 3. Drives 3D facial animation (blend shapes)
// 4. Performs lip sync via audio analysis
// 5. Reports interaction events back to SPS

using System;
using System.Collections;
using System.Net.Http;
using System.Text;
using System.Threading.Tasks;
using Newtonsoft.Json;
using UnityEngine;

namespace SPS.Companion
{
    /// <summary>
    /// Emotion states the companion can express.
    /// Maps to blend shape targets on the 3D face model.
    /// </summary>
    public enum CompanionEmotion
    {
        Neutral,
        Happy,
        Sad,
        Angry,
        Surprised,
        Thinking,
        Listening,
        Speaking
    }

    /// <summary>
    /// Main controller for the SPS Emotional Companion.
    /// Attach to the root GameObject of the companion 3D model.
    /// </summary>
    public class CompanionController : MonoBehaviour
    {
        [Header("SPS Kernel Connection")]
        [Tooltip("URL of the SPS kernel HTTP API")]
        public string SpsServerUrl = "http://localhost:7780";

        [Header("3D Model References")]
        [Tooltip("SkinnedMeshRenderer with blend shapes for facial expressions")]
        public SkinnedMeshRenderer FaceMesh;

        [Tooltip("Animator component for body gestures")]
        public Animator BodyAnimator;

        [Header("Blend Shape Indices")]
        public int HappyBlendShape = 0;
        public int SadBlendShape = 1;
        public int AngryBlendShape = 2;
        public int SurprisedBlendShape = 3;
        public int ThinkingBlendShape = 4;
        public int MouthOpenBlendShape = 5;
        public int LeftEyeBlink = 6;
        public int RightEyeBlink = 7;

        [Header("Lip Sync")]
        [Tooltip("AudioSource for TTS playback")]
        public AudioSource TtsAudioSource;
        [Tooltip("Smoothing factor for lip sync (0-1)")]
        [Range(0.1f, 0.99f)]
        public float LipSyncSmoothing = 0.8f;

        [Header("Animation Settings")]
        public float EmotionTransitionSpeed = 2.0f;
        public float IdleBlinkInterval = 3.0f;

        // Private state
        private CompanionEmotion _currentEmotion = CompanionEmotion.Neutral;
        private float _targetMouthOpen = 0f;
        private float _currentMouthOpen = 0f;
        private float[] _targetBlendShapes;
        private float[] _currentBlendShapes;
        private float _nextBlinkTime;
        private HttpClient _httpClient;
        private string _goalId;

        // ─── Unity Lifecycle ─────────────────────────────────────────────

        void Start()
        {
            _targetBlendShapes = new float[8];
            _currentBlendShapes = new float[8];
            _httpClient = new HttpClient { BaseAddress = new Uri(SpsServerUrl) };
            _nextBlinkTime = Time.time + IdleBlinkInterval + UnityEngine.Random.Range(-1f, 1f);

            // Start the companion loop (poll SPS for commands).
            StartCoroutine(CompanionLoop());

            Debug.Log("[SPS Companion] Controller initialized, connecting to " + SpsServerUrl);
        }

        void Update()
        {
            UpdateBlendShapes();
            UpdateLipSync();
            UpdateBlinking();
        }

        void OnDestroy()
        {
            _httpClient?.Dispose();
        }

        // ─── Emotion Control ─────────────────────────────────────────────

        /// <summary>
        /// Set the companion's emotional expression.
        /// Transitions smoothly to the new emotion.
        /// </summary>
        public void SetEmotion(CompanionEmotion emotion)
        {
            if (_currentEmotion == emotion) return;
            _currentEmotion = emotion;
            Debug.Log($"[SPS Companion] Emotion → {emotion}");

            // Reset all blend shape targets.
            for (int i = 0; i < _targetBlendShapes.Length; i++)
                _targetBlendShapes[i] = 0f;

            // Set the target for the new emotion.
            switch (emotion)
            {
                case CompanionEmotion.Happy:
                    _targetBlendShapes[HappyBlendShape] = 80f;
                    break;
                case CompanionEmotion.Sad:
                    _targetBlendShapes[SadBlendShape] = 70f;
                    break;
                case CompanionEmotion.Angry:
                    _targetBlendShapes[AngryBlendShape] = 75f;
                    break;
                case CompanionEmotion.Surprised:
                    _targetBlendShapes[SurprisedBlendShape] = 90f;
                    break;
                case CompanionEmotion.Thinking:
                    _targetBlendShapes[ThinkingBlendShape] = 60f;
                    break;
                case CompanionEmotion.Listening:
                    _targetBlendShapes[HappyBlendShape] = 20f;
                    break;
                case CompanionEmotion.Speaking:
                    // Mouth animation handled by lip sync.
                    break;
            }

            // Trigger body animation.
            if (BodyAnimator != null)
            {
                BodyAnimator.SetTrigger(emotion.ToString());
            }
        }

        // ─── Lip Sync ────────────────────────────────────────────────────

        /// <summary>
        /// Play TTS audio and drive lip sync from the audio spectrum.
        /// </summary>
        public void Speak(string audioBase64)
        {
            // Decode base64 audio and play.
            byte[] audioBytes = Convert.FromBase64String(audioBase64);
            AudioClip clip = WavToAudioClip(audioBytes);
            if (clip != null && TtsAudioSource != null)
            {
                TtsAudioSource.clip = clip;
                TtsAudioSource.Play();
                SetEmotion(CompanionEmotion.Speaking);
                Debug.Log("[SPS Companion] Speaking...");
            }
        }

        private void UpdateLipSync()
        {
            if (TtsAudioSource != null && TtsAudioSource.isPlaying)
            {
                // Get audio spectrum data.
                float[] spectrum = new float[256];
                TtsAudioSource.GetSpectrumData(spectrum, 0, FFTWindow.BlackmanHarris);

                // Compute average energy in speech frequency range.
                float energy = 0f;
                for (int i = 10; i < 80; i++) // ~200Hz-1600Hz
                    energy += spectrum[i];
                energy /= 70f;

                _targetMouthOpen = Mathf.Clamp(energy * 500f, 0f, 100f);
            }
            else
            {
                _targetMouthOpen = 0f;
                if (_currentEmotion == CompanionEmotion.Speaking)
                    SetEmotion(CompanionEmotion.Neutral);
            }

            // Smooth the mouth open value.
            _currentMouthOpen = Mathf.Lerp(
                _currentMouthOpen,
                _targetMouthOpen,
                (1f - LipSyncSmoothing) * Time.deltaTime * 30f
            );

            if (FaceMesh != null)
            {
                FaceMesh.SetBlendShapeWeight(MouthOpenBlendShape, _currentMouthOpen);
            }
        }

        // ─── Blinking ────────────────────────────────────────────────────

        private void UpdateBlinking()
        {
            if (Time.time > _nextBlinkTime)
            {
                StartCoroutine(BlinkCoroutine());
                _nextBlinkTime = Time.time + IdleBlinkInterval + UnityEngine.Random.Range(-1f, 2f);
            }
        }

        private IEnumerator BlinkCoroutine()
        {
            float blinkDuration = 0.15f;
            float halfDuration = blinkDuration / 2f;

            // Close eyes.
            float elapsed = 0f;
            while (elapsed < halfDuration)
            {
                float t = elapsed / halfDuration;
                if (FaceMesh != null)
                {
                    FaceMesh.SetBlendShapeWeight(LeftEyeBlink, t * 100f);
                    FaceMesh.SetBlendShapeWeight(RightEyeBlink, t * 100f);
                }
                elapsed += Time.deltaTime;
                yield return null;
            }

            // Open eyes.
            elapsed = 0f;
            while (elapsed < halfDuration)
            {
                float t = elapsed / halfDuration;
                if (FaceMesh != null)
                {
                    FaceMesh.SetBlendShapeWeight(LeftEyeBlink, (1f - t) * 100f);
                    FaceMesh.SetBlendShapeWeight(RightEyeBlink, (1f - t) * 100f);
                }
                elapsed += Time.deltaTime;
                yield return null;
            }
        }

        // ─── Blend Shape Interpolation ───────────────────────────────────

        private void UpdateBlendShapes()
        {
            if (FaceMesh == null) return;

            for (int i = 0; i < _targetBlendShapes.Length; i++)
            {
                if (i == MouthOpenBlendShape) continue; // Handled by lip sync.
                float current = _currentBlendShapes[i];
                float target = _targetBlendShapes[i];
                float newValue = Mathf.Lerp(current, target, EmotionTransitionSpeed * Time.deltaTime);
                _currentBlendShapes[i] = newValue;
                FaceMesh.SetBlendShapeWeight(i, newValue);
            }
        }

        // ─── SPS Kernel Communication ────────────────────────────────────

        /// <summary>
        /// Main companion loop — polls SPS kernel for commands.
        /// </summary>
        private IEnumerator CompanionLoop()
        {
            while (true)
            {
                yield return new WaitForSeconds(1f);

                // Check for pending companion commands.
                var task = PollCompanionCommands();
                yield return new WaitUntil(() => task.IsCompleted);

                if (task.Exception != null)
                {
                    Debug.LogWarning($"[SPS Companion] Poll failed: {task.Exception.Message}");
                    continue;
                }

                var response = task.Result;
                if (response != null)
                {
                    ProcessCompanionCommand(response);
                }
            }
        }

        private async Task<string> PollCompanionCommands()
        {
            try
            {
                var response = await _httpClient.GetAsync("/api/companion/active");
                if (response.IsSuccessStatusCode)
                {
                    return await response.Content.ReadAsStringAsync();
                }
            }
            catch (Exception e)
            {
                Debug.LogWarning($"[SPS Companion] HTTP error: {e.Message}");
            }
            return null;
        }

        private void ProcessCompanionCommand(string jsonResponse)
        {
            try
            {
                var data = JsonConvert.DeserializeObject<dynamic>(jsonResponse);
                if (data?.active_goals != null)
                {
                    foreach (var goal in data.active_goals)
                    {
                        string goalId = goal.goal_id;
                        string emotion = goal.emotion ?? "neutral";

                        // Update companion emotion based on goal state.
                        SetEmotion(ParseEmotion(emotion));
                    }
                }
            }
            catch (Exception e)
            {
                Debug.LogWarning($"[SPS Companion] Parse error: {e.Message}");
            }
        }

        private CompanionEmotion ParseEmotion(string emotion)
        {
            return emotion.ToLower() switch
            {
                "happy" => CompanionEmotion.Happy,
                "sad" => CompanionEmotion.Sad,
                "angry" => CompanionEmotion.Angry,
                "surprised" => CompanionEmotion.Surprised,
                "thinking" => CompanionEmotion.Thinking,
                "listening" => CompanionEmotion.Listening,
                "speaking" => CompanionEmotion.Speaking,
                _ => CompanionEmotion.Neutral,
            };
        }

        // ─── Utilities ───────────────────────────────────────────────────

        private AudioClip WavToAudioClip(byte[] wavBytes)
        {
            // Minimal WAV parser — converts PCM 16-bit mono to AudioClip.
            if (wavBytes.Length < 44) return null;

            int channels = BitConverter.ToInt16(wavBytes, 22);
            int frequency = BitConverter.ToInt32(wavBytes, 24);
            int dataSize = wavBytes.Length - 44;

            float[] samples = new float[dataSize / 2];
            for (int i = 0; i < samples.Length; i++)
            {
                short sample = BitConverter.ToInt16(wavBytes, 44 + i * 2);
                samples[i] = sample / 32768f;
            }

            AudioClip clip = AudioClip.Create("TTS", samples.Length, channels, frequency, false);
            clip.SetData(samples, 0);
            return clip;
        }

        /// <summary>
        /// Send a heartbeat to the SPS kernel.
        /// </summary>
        public async Task SendHeartbeat(string goalId, string review)
        {
            try
            {
                var payload = new
                {
                    goal_id = goalId,
                    review = review
                };
                var json = JsonConvert.SerializeObject(payload);
                var content = new StringContent(json, Encoding.UTF8, "application/json");
                await _httpClient.PostAsync("/api/companion/heartbeat", content);
                Debug.Log("[SPS Companion] Heartbeat sent");
            }
            catch (Exception e)
            {
                Debug.LogWarning($"[SPS Companion] Heartbeat failed: {e.Message}");
            }
        }
    }
}
