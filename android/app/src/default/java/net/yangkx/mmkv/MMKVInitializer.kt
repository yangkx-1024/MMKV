package net.yangkx.mmkv

import android.content.Context
import net.yangkx.mmkv.log.LogLevel

object MMKVInitializer {
    fun init(context: Context): MMKV {
        val dir = context.getDir("mmkv", Context.MODE_PRIVATE)
        MMKV.setLogLevel(LogLevel.VERBOSE)
        return MMKV(dir.absolutePath)
    }
}