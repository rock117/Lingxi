package com.lingxi.app.data.api

import com.lingxi.app.data.model.Condition
import com.lingxi.app.data.model.CreateConditionRequest
import com.lingxi.app.data.model.FundStatsMeta
import com.lingxi.app.data.model.FundStatsResult
import com.lingxi.app.data.model.HealthResponse
import com.lingxi.app.data.model.MsiResult
import com.lingxi.app.data.model.MttResult
import com.lingxi.app.data.model.NotificationItem
import com.lingxi.app.data.model.ScreenRequest
import com.lingxi.app.data.model.ScreenResult
import com.lingxi.app.data.model.StockDetail
import com.lingxi.app.data.model.StrategyDef
import com.lingxi.app.data.model.UpdateConditionRequest
import retrofit2.http.Body
import retrofit2.http.DELETE
import retrofit2.http.GET
import retrofit2.http.POST
import retrofit2.http.PUT
import retrofit2.http.Path
import retrofit2.http.Query

/**
 * 后端 REST 接口。
 *
 * 行情与通知一律走后端真实行情 / WebSocket 推送。
 */
interface LingxiApi {
    @GET("api/health")
    suspend fun health(): HealthResponse

    @GET("api/market/sentiment")
    suspend fun sentiment(
        @Query("days") days: Int = 5,
    ): MsiResult

    @GET("api/market/mid-term-trend")
    suspend fun midTermTrend(): MttResult

    @GET("api/conditions")
    suspend fun listConditions(): List<Condition>

    @GET("api/conditions/{id}")
    suspend fun getCondition(@Path("id") id: Long): Condition

    @POST("api/conditions")
    suspend fun createCondition(@Body body: CreateConditionRequest): Condition

    @PUT("api/conditions/{id}")
    suspend fun updateCondition(
        @Path("id") id: Long,
        @Body body: UpdateConditionRequest,
    ): Condition

    @DELETE("api/conditions/{id}")
    suspend fun deleteCondition(@Path("id") id: Long)

    @GET("api/market/strategies")
    suspend fun listStrategies(): List<StrategyDef>

    @POST("api/market/screen")
    suspend fun screenPreview(@Body body: ScreenRequest): ScreenResult

    @GET("api/market/screen/{id}")
    suspend fun screenByCondition(
        @Path("id") id: Long,
        @Query("top_n") topN: Int = 50,
    ): ScreenResult

    @GET("api/market/stock")
    suspend fun stockDetail(@Query("symbol") symbol: String): StockDetail

    @GET("api/market/fund-stats/meta")
    suspend fun fundStatsMeta(): FundStatsMeta

    @GET("api/market/fund-stats")
    suspend fun fundStats(
        @Query("year") year: Int? = null,
        @Query("period") period: String? = null,
    ): FundStatsResult

    @GET("api/notifications")
    suspend fun listNotifications(
        @Query("unread") unread: Boolean? = null,
    ): List<NotificationItem>

    @POST("api/notifications/{id}/read")
    suspend fun markRead(@Path("id") id: Long): NotificationItem

    @DELETE("api/notifications/{id}")
    suspend fun deleteNotification(@Path("id") id: Long)

    @DELETE("api/notifications")
    suspend fun deleteAllNotifications()
}
