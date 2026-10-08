package com.lingxi.app

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.compose.runtime.Composable
import androidx.core.content.ContextCompat
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.lingxi.app.condition.ConditionFormScreen
import com.lingxi.app.home.HomeScreen
import com.lingxi.app.navigation.Routes
import com.lingxi.app.notification.NotificationHelper
import com.lingxi.app.service.LingxiService
import com.lingxi.app.settings.SettingsScreen
import com.lingxi.app.theme.LingxiTheme
import com.lingxi.app.viewmodel.ConditionViewModel
import com.lingxi.app.viewmodel.MarketViewModel
import com.lingxi.app.viewmodel.NotificationViewModel
import com.lingxi.app.viewmodel.SettingsViewModel

class MainActivity : ComponentActivity() {
    private val marketViewModel: MarketViewModel by viewModels()
    private val conditionViewModel: ConditionViewModel by viewModels()
    private val notificationViewModel: NotificationViewModel by viewModels()
    private val settingsViewModel: SettingsViewModel by viewModels()

    private val requestNotificationPermission =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
            if (granted) {
                LingxiService.start(this)
            }
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        // 注册通知渠道
        NotificationHelper.ensureChannels(this)

        // 请求通知权限并启动前台服务
        startNotificationService()

        setContent {
            LingxiTheme {
                LingxiNav(
                    marketViewModel = marketViewModel,
                    conditionViewModel = conditionViewModel,
                    notificationViewModel = notificationViewModel,
                    settingsViewModel = settingsViewModel,
                )
            }
        }
    }

    private fun startNotificationService() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            val granted = ContextCompat.checkSelfPermission(
                this,
                Manifest.permission.POST_NOTIFICATIONS,
            ) == PackageManager.PERMISSION_GRANTED

            if (granted) {
                LingxiService.start(this)
            } else {
                requestNotificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
            }
        } else {
            LingxiService.start(this)
        }
    }

    override fun onDestroy() {
        // 不在 onDestroy 停止服务，让后台监听持续运行
        // 用户可通过系统通知设置手动停止
        super.onDestroy()
    }
}

@Composable
private fun LingxiNav(
    marketViewModel: MarketViewModel,
    conditionViewModel: ConditionViewModel,
    notificationViewModel: NotificationViewModel,
    settingsViewModel: SettingsViewModel,
) {
    val navController = rememberNavController()

    NavHost(navController = navController, startDestination = Routes.HOME) {
        composable(Routes.HOME) {
            HomeScreen(
                marketViewModel = marketViewModel,
                conditionViewModel = conditionViewModel,
                notificationViewModel = notificationViewModel,
                onOpenSettings = { navController.navigate(Routes.SETTINGS) },
                onCreateCondition = { navController.navigate(Routes.CONDITION_CREATE) },
                onEditCondition = { id -> navController.navigate(Routes.conditionEdit(id)) },
            )
        }
        composable(Routes.SETTINGS) {
            SettingsScreen(
                viewModel = settingsViewModel,
                onBack = { navController.popBackStack() },
            )
        }
        composable(Routes.CONDITION_CREATE) {
            ConditionFormScreen(
                viewModel = conditionViewModel,
                editId = null,
                onDone = { navController.popBackStack() },
                onBack = { navController.popBackStack() },
            )
        }
        composable(
            route = Routes.CONDITION_EDIT,
            arguments = listOf(navArgument("id") { type = NavType.LongType }),
        ) { entry ->
            val id = entry.arguments?.getLong("id")
            ConditionFormScreen(
                viewModel = conditionViewModel,
                editId = id,
                onDone = { navController.popBackStack() },
                onBack = { navController.popBackStack() },
            )
        }
    }
}
