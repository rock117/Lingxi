package com.lingxi.app.data.model

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class EtfSharePoint(
    val date: String = "",
    val share: Double = 0.0,
    val mkt: Double = 0.0,
)

@Serializable
data class EtfShareMember(
    val symbol: String = "",
    val name: String = "",
    val share: Double = 0.0,
    val mkt: Double? = null,
    @SerialName("delta_share") val deltaShare: Double? = null,
    @SerialName("delta_share_pct") val deltaSharePct: Double? = null,
    @SerialName("delta_mkt") val deltaMkt: Double? = null,
    @SerialName("delta_mkt_pct") val deltaMktPct: Double? = null,
)

@Serializable
data class EtfShareGroup(
    val id: String = "",
    val label: String = "",
    @SerialName("latest_share") val latestShare: Double = 0.0,
    @SerialName("latest_mkt") val latestMkt: Double? = null,
    @SerialName("delta_share") val deltaShare: Double? = null,
    @SerialName("delta_share_pct") val deltaSharePct: Double? = null,
    @SerialName("delta_mkt") val deltaMkt: Double? = null,
    @SerialName("delta_mkt_pct") val deltaMktPct: Double? = null,
    val series: List<EtfSharePoint> = emptyList(),
    val members: List<EtfShareMember> = emptyList(),
)

@Serializable
data class EtfShareGroupsResult(
    @SerialName("as_of") val asOf: String = "",
    val range: String = "3m",
    val granularity: String = "day",
    @SerialName("start_date") val startDate: String = "",
    val note: String = "",
    val groups: List<EtfShareGroup> = emptyList(),
)
