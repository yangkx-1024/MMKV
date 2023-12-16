package net.yangkx.mmkv

import android.content.Context
import net.yangkx.mmkv.log.LogLevel

object MMKVInitializer {
    fun init(context: Context) {
        val dir = context.getDir("mmkv", Context.MODE_PRIVATE)
        MMKV.initialize(dir.absolutePath)
        MMKV.setLogLevel(LogLevel.VERBOSE)
    }
}