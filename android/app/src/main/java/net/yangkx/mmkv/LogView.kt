package net.yangkx.mmkv

import android.util.Log
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalInspectionMode
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import net.yangkx.mmkv.log.LogLevel
import net.yangkx.mmkv.log.Logger

@Composable
fun LogView(leadText: String, logLevel: LogLevel) {
    val logState = rememberLogState(leadText, logLevel)
    Text(
        text = logState.logText,
        modifier = Modifier
            .fillMaxWidth()
            .background(Color.Black)
            .verticalScroll(logState.scrollState, true)
            .padding(8.dp),
        color = Color.Green,
        fontSize = 12.sp,
        lineHeight = 18.sp
    )
}


@Composable
private fun rememberLogState(
    leadText: String,
    logLevel: LogLevel
): LogState {
    val isPreview = LocalInspectionMode.current
    val scrollState = rememberScrollState()
    val coroutineScope = rememberCoroutineScope()
    return remember {
        LogState(
            leadText,
            logLevel,
            scrollState,
            coroutineScope,
            isPreview
        )
    }
}

@Stable
private class LogState(
    leadText: String,
    logLevel: LogLevel,
    val scrollState: ScrollState,
    private val coroutineScope: CoroutineScope,
    isPreview: Boolean = false
) : Logger {
    companion object {
        private const val TAG = "MMKV-APP"
    }

    init {
        if (!isPreview) {
            MMKV.setLogger(this)
            MMKV.setLogLevel(logLevel)
        }
    }
    var logText by mutableStateOf(leadText + "\n")
    override fun verbose(log: String) {
        Log.v(TAG, log)
        appendLog("V - $log\n")
    }

    override fun info(log: String) {
        Log.i(TAG, log)
        appendLog("I - $log\n")
    }

    override fun debug(log: String) {
        Log.d(TAG, log)
        appendLog("D - $log\n")
    }

    override fun warn(log: String) {
        Log.w(TAG, log)
        appendLog("W - $log\n")
    }

    override fun error(log: String) {
        Log.e(TAG, log)
        appendLog("E - $log\n")
    }

    private fun appendLog(text: String) {
        logText += text
        coroutineScope.launch {
            scrollState.animateScrollTo(Int.MAX_VALUE)
        }
    }
}

@Preview
@Composable
fun LogViewPreview() {
    LogView("MMKV Log:", LogLevel.VERBOSE)
}