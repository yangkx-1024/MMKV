package net.yangkx.mmkv

import android.content.Context
import net.yangkx.mmkv.log.LogLevel

object MMKVInitializer {
    fun init(context: Context): MMKV {
        val dir = context.getDir("mmkv", Context.MODE_PRIVATE)
        MMKV.setLogLevel(LogLevel.DEBUG)
        return MMKV(dir.absolutePath, "88C51C536176AD8A8EE4A06F62EE897E")
    }
}