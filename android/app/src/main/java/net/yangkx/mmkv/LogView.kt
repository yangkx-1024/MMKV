package net.yangkx.mmkv

import android.util.Log
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalInspectionMode
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import net.yangkx.mmkv.log.Logger

@Composable
fun LogView(leadText: String) {
    val logState = rememberLogState(leadText)
    LazyColumn(
        state = logState.listState,
        modifier = Modifier
            .fillMaxWidth()
            .background(Color.Black)
            .size(Dp.Infinity, 200.dp)
            .padding(8.dp)
    ) {
        items(logState.logList) {
            Text(
                text = it,
                modifier = Modifier.fillMaxWidth(),
                color = Color.Green,
                fontSize = 12.sp,
            )
        }
    }
}


@Composable
private fun rememberLogState(
    leadText: String,
): LogState {
    val isPreview = LocalInspectionMode.current
    val listState = rememberLazyListState()
    val coroutineScope = rememberCoroutineScope()
    return remember {
        LogState(
            leadText, listState, coroutineScope, isPreview
        )
    }
}

@Stable
private class LogState(
    leadText: String,
    val listState: LazyListState,
    private val coroutineScope: CoroutineScope,
    isPreview: Boolean = false
) : Logger {
    companion object {
        private const val TAG = "MMKV-APP"
    }

    init {
        if (!isPreview) {
            MMKV.setLogger(this)
        }
    }

    var logList = mutableStateListOf(leadText)
    override fun verbose(log: String) {
        Log.v(TAG, log)
        appendLog("V - $log")
    }

    override fun info(log: String) {
        Log.i(TAG, log)
        appendLog("I - $log")
    }

    override fun debug(log: String) {
        Log.d(TAG, log)
        appendLog("D - $log")
    }

    override fun warn(log: String) {
        Log.w(TAG, log)
        appendLog("W - $log")
    }

    override fun error(log: String) {
        Log.e(TAG, log)
        appendLog("E - $log")
    }

    private fun appendLog(text: String) {
        logList.add(text)
        coroutineScope.launch {
            listState.animateScrollToItem(logList.size - 1)
        }
    }
}

@Preview
@Composable
fun LogViewPreview() {
    LogView("MMKV Log:")
}