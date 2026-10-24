package com.lingxi.app.data.api

import com.jakewharton.retrofit2.converter.kotlinx.serialization.asConverterFactory
import com.lingxi.app.data.model.ServerConfig
import kotlinx.serialization.json.Json
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import retrofit2.Retrofit
import java.util.concurrent.TimeUnit

/** Retrofit / OkHttp 工厂：`ApiClient.create(serverConfig)` → [LingxiApi]。 */
object ApiClient {
    private val json = Json {
        ignoreUnknownKeys = true
        isLenient = true
    }

    private val okHttp = OkHttpClient.Builder()
        .connectTimeout(10, TimeUnit.SECONDS)
        // 个股 AI 分析含明细翻页 + 模型推理，整体常超过 1 分钟
        .readTimeout(180, TimeUnit.SECONDS)
        .writeTimeout(60, TimeUnit.SECONDS)
        .callTimeout(180, TimeUnit.SECONDS)
        .build()

    fun create(config: ServerConfig): LingxiApi {
        val retrofit = Retrofit.Builder()
            .baseUrl(config.httpBaseUrl())
            .client(okHttp)
            .addConverterFactory(json.asConverterFactory("application/json".toMediaType()))
            .build()
        return retrofit.create(LingxiApi::class.java)
    }
}
