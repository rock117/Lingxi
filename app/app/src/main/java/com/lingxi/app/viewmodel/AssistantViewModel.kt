package com.lingxi.app.viewmodel

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.lingxi.app.data.api.ApiClient
import com.lingxi.app.data.api.AssistantSseClient
import com.lingxi.app.data.api.AssistantSseEvent
import com.lingxi.app.data.model.AiStatus
import com.lingxi.app.data.model.AssistantChatRequest
import com.lingxi.app.data.model.AssistantSession
import com.lingxi.app.data.model.AssistantStep
import com.lingxi.app.data.model.ChatUiMessage
import com.lingxi.app.data.prefs.ServerPrefs
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.decodeFromJsonElement
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import java.util.UUID

data class AssistantUiState(
    val loading: Boolean = false,
    val sending: Boolean = false,
    val status: AiStatus? = null,
    val sessionId: Long? = null,
    val messages: List<ChatUiMessage> = emptyList(),
    val sessions: List<AssistantSession> = emptyList(),
    val draft: String = "",
    val error: String? = null,
    val showHistory: Boolean = false,
)

class AssistantViewModel(application: Application) : AndroidViewModel(application) {
    private val prefs = ServerPrefs(application)
    private val payloadJson = Json { ignoreUnknownKeys = true; isLenient = true }

    private val _ui = MutableStateFlow(AssistantUiState())
    val ui: StateFlow<AssistantUiState> = _ui.asStateFlow()

    private var streamJob: Job? = null

    fun bootstrap(prefill: String? = null) {
        viewModelScope.launch {
            if (!prefill.isNullOrBlank()) {
                _ui.update { it.copy(draft = prefill) }
            }
            refreshStatus()
            refreshSessions()
        }
    }

    fun setDraft(text: String) {
        _ui.update { it.copy(draft = text) }
    }

    fun toggleHistory(show: Boolean? = null) {
        _ui.update { it.copy(showHistory = show ?: !it.showHistory) }
        if (_ui.value.showHistory) refreshSessions()
    }

    fun newChat() {
        stop()
        _ui.update {
            it.copy(
                sessionId = null,
                messages = emptyList(),
                draft = "",
                error = null,
                sending = false,
            )
        }
    }

    fun openSession(id: Long) {
        viewModelScope.launch {
            _ui.update { it.copy(loading = true, error = null, showHistory = false) }
            try {
                val api = ApiClient.create(prefs.configFlow.first())
                val detail = api.aiSession(id)
                val msgs = detail.messages.map { m ->
                    val steps = parseSteps(m.toolPayload)
                    val suggestions = parseSuggestions(m.toolPayload)
                    ChatUiMessage(
                        localId = "srv-${m.id}",
                        role = m.role,
                        content = m.content,
                        steps = steps,
                        suggestions = suggestions,
                        streaming = false,
                        stepsExpanded = false,
                    )
                }
                _ui.update {
                    it.copy(
                        loading = false,
                        sessionId = detail.session.id,
                        messages = msgs,
                    )
                }
            } catch (e: Exception) {
                _ui.update { it.copy(loading = false, error = e.message ?: "加载会话失败") }
            }
        }
    }

    fun deleteSession(id: Long) {
        viewModelScope.launch {
            try {
                ApiClient.create(prefs.configFlow.first()).deleteAiSession(id)
                if (_ui.value.sessionId == id) newChat()
                refreshSessions()
            } catch (e: Exception) {
                _ui.update { it.copy(error = e.message ?: "删除失败") }
            }
        }
    }

    fun toggleSteps(localId: String) {
        _ui.update { st ->
            st.copy(
                messages = st.messages.map {
                    if (it.localId == localId) it.copy(stepsExpanded = !it.stepsExpanded) else it
                },
            )
        }
    }

    fun send(text: String? = null) {
        val content = (text ?: _ui.value.draft).trim()
        if (content.isEmpty() || _ui.value.sending) return
        if (_ui.value.status?.aiEnabled == false) {
            _ui.update { it.copy(error = "助手未启用（服务端未配置 AI）") }
            return
        }

        val userMsg = ChatUiMessage(
            localId = UUID.randomUUID().toString(),
            role = "user",
            content = content,
        )
        val assistantId = UUID.randomUUID().toString()
        val placeholder = ChatUiMessage(
            localId = assistantId,
            role = "assistant",
            content = "",
            streaming = true,
            stepsExpanded = true,
        )
        _ui.update {
            it.copy(
                draft = "",
                sending = true,
                error = null,
                messages = it.messages + userMsg + placeholder,
            )
        }

        streamJob?.cancel()
        streamJob = viewModelScope.launch {
            val cfg = prefs.configFlow.first()
            val req = AssistantChatRequest(sessionId = _ui.value.sessionId, message = content)
            try {
                AssistantSseClient.stream(cfg, req).collect { ev ->
                    when (ev) {
                        is AssistantSseEvent.Session -> {
                            _ui.update { it.copy(sessionId = ev.sessionId) }
                        }
                        is AssistantSseEvent.Step -> upsertStep(assistantId, ev.step)
                        is AssistantSseEvent.Delta -> appendDelta(assistantId, ev.text)
                        is AssistantSseEvent.Done -> {
                            _ui.update { st ->
                                st.copy(
                                    sessionId = ev.sessionId,
                                    sending = false,
                                    messages = st.messages.map {
                                        if (it.localId != assistantId) it
                                        else it.copy(
                                            steps = ev.steps.ifEmpty { it.steps },
                                            suggestions = ev.suggestions,
                                            streaming = false,
                                            stepsExpanded = false,
                                        )
                                    },
                                )
                            }
                            refreshSessions()
                        }
                        is AssistantSseEvent.Error -> {
                            _ui.update { st ->
                                st.copy(
                                    sending = false,
                                    error = ev.message,
                                    messages = st.messages.map {
                                        if (it.localId != assistantId) it
                                        else it.copy(
                                            content = it.content.ifBlank { ev.message },
                                            streaming = false,
                                        )
                                    },
                                )
                            }
                        }
                    }
                }
            } catch (e: Exception) {
                _ui.update {
                    it.copy(sending = false, error = e.message ?: "发送失败")
                }
            } finally {
                _ui.update { it.copy(sending = false) }
                // 确保占位不再 streaming
                _ui.update { st ->
                    st.copy(
                        messages = st.messages.map {
                            if (it.localId == assistantId) it.copy(streaming = false) else it
                        },
                    )
                }
            }
        }
    }

    fun stop() {
        AssistantSseClient.cancel()
        streamJob?.cancel()
        streamJob = null
        _ui.update { st ->
            st.copy(
                sending = false,
                messages = st.messages.map {
                    if (it.streaming) {
                        it.copy(
                            streaming = false,
                            content = it.content.ifBlank { "已停止生成。需要的话可以换个问题再问。" },
                        )
                    } else it
                },
            )
        }
    }

    private fun upsertStep(assistantId: String, step: AssistantStep) {
        _ui.update { st ->
            st.copy(
                messages = st.messages.map { msg ->
                    if (msg.localId != assistantId) msg
                    else {
                        val steps = msg.steps.toMutableList()
                        val idx = steps.indexOfFirst { it.id == step.id }
                        if (idx >= 0) steps[idx] = step else steps.add(step)
                        msg.copy(steps = steps, stepsExpanded = true)
                    }
                },
            )
        }
    }

    private fun appendDelta(assistantId: String, text: String) {
        if (text.isEmpty()) return
        _ui.update { st ->
            st.copy(
                messages = st.messages.map {
                    if (it.localId != assistantId) it
                    else it.copy(content = it.content + text)
                },
            )
        }
    }

    private fun parseSteps(payload: kotlinx.serialization.json.JsonElement?): List<AssistantStep> {
        val obj = payload?.jsonObject ?: return emptyList()
        val arr = obj["steps"]?.jsonArray ?: return emptyList()
        return arr.mapNotNull {
            runCatching { payloadJson.decodeFromJsonElement<AssistantStep>(it) }.getOrNull()
        }
    }

    private fun parseSuggestions(payload: kotlinx.serialization.json.JsonElement?): List<String> {
        val obj = payload?.jsonObject ?: return emptyList()
        val arr = obj["suggestions"]?.jsonArray ?: return emptyList()
        return arr.mapNotNull { it.jsonPrimitive.contentOrNull?.takeIf(String::isNotBlank) }
    }

    private suspend fun refreshStatus() {
        try {
            val st = ApiClient.create(prefs.configFlow.first()).aiStatus()
            _ui.update { it.copy(status = st) }
        } catch (e: Exception) {
            _ui.update {
                it.copy(
                    status = AiStatus(aiEnabled = false),
                    error = e.message,
                )
            }
        }
    }

    private fun refreshSessions() {
        viewModelScope.launch {
            try {
                val list = ApiClient.create(prefs.configFlow.first()).aiSessions()
                _ui.update { it.copy(sessions = list) }
            } catch (_: Exception) {
            }
        }
    }
}
