// SPS Emotional Companion — Main Scene Setup
// Phase 14: Attach this to a root GameObject in the main scene.
// It wires all companion components together.

using UnityEngine;

namespace SPS.Companion
{
    /// <summary>
    /// Bootstraps the SPS Emotional Companion scene.
    /// Automatically wires CompanionController + EmotionManager +
    /// TtsHandler + SttHandler + ConversationManager.
    /// </summary>
    public class CompanionBootstrap : MonoBehaviour
    {
        [Header("Auto-Configure")]
        [Tooltip("If true, automatically create + wire all components on Start")]
        public bool AutoConfigure = true;

        [Header("SPS Server")]
        public string SpsServerUrl = "http://localhost:7780";

        [Header("3D Model")]
        [Tooltip("Assign the SkinnedMeshRenderer with blend shapes")]
        public SkinnedMeshRenderer FaceMesh;
        public Animator BodyAnimator;
        public AudioSource TtsAudioSource;

        [Header("Auto-created Components (read-only)")]
        [SerializeField] private CompanionController _companion;
        [SerializeField] private EmotionManager _emotion;
        [SerializeField] private TtsHandler _tts;
        [SerializeField] private SttHandler _stt;
        [SerializeField] private ConversationManager _conversation;

        void Start()
        {
            if (AutoConfigure)
            {
                ConfigureAll();
            }
        }

        /// <summary>
        /// Create and wire all companion components.
        /// </summary>
        public void ConfigureAll()
        {
            // CompanionController.
            if (_companion == null)
            {
                _companion = gameObject.AddComponent<CompanionController>();
                _companion.FaceMesh = FaceMesh;
                _companion.BodyAnimator = BodyAnimator;
                _companion.TtsAudioSource = TtsAudioSource;
                _companion.SpsServerUrl = SpsServerUrl;
            }

            // EmotionManager.
            if (_emotion == null)
            {
                _emotion = gameObject.AddComponent<EmotionManager>();
                _emotion.Companion = _companion;
            }

            // TtsHandler.
            if (_tts == null)
            {
                _tts = gameObject.AddComponent<TtsHandler>();
                _tts.SpsServerUrl = SpsServerUrl;
                _tts.AudioSource = TtsAudioSource;
            }

            // SttHandler.
            if (_stt == null)
            {
                _stt = gameObject.AddComponent<SttHandler>();
                _stt.SpsServerUrl = SpsServerUrl;
            }

            // ConversationManager.
            if (_conversation == null)
            {
                _conversation = gameObject.AddComponent<ConversationManager>();
                _conversation.Stt = _stt;
                _conversation.Tts = _tts;
                _conversation.Emotion = _emotion;
                _conversation.Companion = _companion;
                _conversation.SpsServerUrl = SpsServerUrl;
            }

            Debug.Log("[SPS Companion] All components configured and wired.");
            Debug.Log($"[SPS Companion] Connected to: {SpsServerUrl}");
        }

        /// <summary>
        /// Start a voice conversation.
        /// Call from UI button or voice activation.
        /// </summary>
        public void StartConversation()
        {
            _conversation?.StartConversation();
        }

        /// <summary>
        /// End the current conversation.
        /// </summary>
        public void EndConversation()
        {
            _conversation?.EndConversation();
        }

        /// <summary>
        /// Send a text message (no voice).
        /// </summary>
        public void SendTextMessage(string message)
        {
            _conversation?.ProcessTextMessage(message);
        }
    }
}
