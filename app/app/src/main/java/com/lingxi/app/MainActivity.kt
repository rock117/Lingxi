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
import com.lingxi.app.condition.ScreenComposeScreen
import com.lingxi.app.condition.ScreenResultScreen
import com.lingxi.app.condition.StockAnalyzeScreen
import com.lingxi.app.home.HomeScreen
import com.lingxi.app.navigation.Routes
import com.lingxi.app.notification.NotificationHelper
import com.lingxi.app.service.LingxiService
import com.lingxi.app.settings.SettingsScreen
import com.lingxi.app.theme.LingxiTheme
import com.lingxi.app.viewmodel.MarketViewModel
import com.lingxi.app.viewmodel.NotificationViewModel
import com.lingxi.app.viewmodel.ScreenViewModel
import com.lingxi.app.viewmodel.SettingsViewModel

class MainActivity : ComponentActivity() {
    private val marketViewModel: MarketViewModel by viewModels()
    private val screenViewModel: ScreenViewModel by viewModels()
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

        NotificationHelper.ensureChannels(this)
        startNotificationService()

        setContent {
            LingxiTheme {
                LingxiNav(
                    marketViewModel = marketViewModel,
                    screenViewModel = screenViewModel,
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
        super.onDestroy()
    }
}

@Composable
private fun LingxiNav(
    marketViewModel: MarketViewModel,
    screenViewModel: ScreenViewModel,
    notificationViewModel: NotificationViewModel,
    settingsViewModel: SettingsViewModel,
) {
    val navController = rememberNavController()

    NavHost(navController = navController, startDestination = Routes.HOME) {
        composable(Routes.HOME) {
            HomeScreen(
                marketViewModel = marketViewModel,
                screenViewModel = screenViewModel,
                notificationViewModel = notificationViewModel,
                onOpenSettings = { navController.navigate(Routes.SETTINGS) },
                onComposeScreen = { typeId ->
                    navController.navigate(Routes.screenCompose(typeId))
                },
                onEditScreen = { id ->
                    navController.navigate(Routes.screenEdit(id))
                },
                onRunScreen = { id ->
                    screenViewModel.clearResult()
                    navController.navigate(Routes.screenResult(id))
                },
            )
        }
        composable(Routes.SETTINGS) {
            SettingsScreen(
                viewModel = settingsViewModel,
                onBack = { navController.popBackStack() },
            )
        }
        composable(
            route = Routes.SCREEN_COMPOSE,
            arguments = listOf(navArgument("typeId") { type = NavType.StringType }),
        ) { entry ->
            val raw = entry.arguments?.getString("typeId")
            val typeId = raw?.takeIf { it.isNotBlank() && it != "_" }
            ScreenComposeScreen(
                viewModel = screenViewModel,
                initialTypeId = typeId,
                editId = null,
                onBack = { navController.popBackStack() },
                onSavedAndRun = { id ->
                    navController.popBackStack()
                    navController.navigate(Routes.screenResult(id))
                },
                onPreviewDone = {
                    navController.navigate(Routes.screenResult(null))
                },
            )
        }
        composable(
            route = Routes.SCREEN_EDIT,
            arguments = listOf(navArgument("id") { type = NavType.LongType }),
        ) { entry ->
            val id = entry.arguments?.getLong("id") ?: 0L
            ScreenComposeScreen(
                viewModel = screenViewModel,
                initialTypeId = null,
                editId = id,
                onBack = { navController.popBackStack() },
                onSavedAndRun = { savedId ->
                    navController.popBackStack()
                    navController.navigate(Routes.screenResult(savedId))
                },
                onPreviewDone = {
                    navController.navigate(Routes.screenResult(null))
                },
            )
        }
        composable(
            route = Routes.SCREEN_RESULT,
            arguments = listOf(navArgument("conditionId") { type = NavType.LongType }),
        ) { entry ->
            val id = entry.arguments?.getLong("conditionId")?.takeIf { it > 0 }
            ScreenResultScreen(
                viewModel = screenViewModel,
                conditionId = id,
                onBack = { navController.popBackStack() },
                onOpenStock = { symbol ->
                    screenViewModel.clearStockAnalyze()
                    navController.navigate(Routes.stockAnalyze(symbol))
                },
            )
        }
        composable(Routes.STOCK_ANALYZE) {
            StockAnalyzeScreen(
                viewModel = screenViewModel,
                initialSymbol = null,
                showBack = true,
                onBack = {
                    screenViewModel.clearStockAnalyze()
                    navController.popBackStack()
                },
            )
        }
        composable(
            route = Routes.STOCK_ANALYZE_SYMBOL,
            arguments = listOf(navArgument("symbol") { type = NavType.StringType }),
        ) { entry ->
            val encoded = entry.arguments?.getString("symbol").orEmpty()
            val symbol = Routes.decodeStockSymbol(encoded)
            StockAnalyzeScreen(
                viewModel = screenViewModel,
                initialSymbol = symbol,
                showBack = true,
                onBack = {
                    screenViewModel.clearStockAnalyze()
                    navController.popBackStack()
                },
            )
        }
    }
}
