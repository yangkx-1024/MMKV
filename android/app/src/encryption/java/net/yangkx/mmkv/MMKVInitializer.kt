package net.yangkx.mmkv

import android.content.Context

object MMKVInitializer {
    fun init(context: Context) {
        val dir = context.getDir("mmkv", Context.MODE_PRIVATE)
        if (dir.exists()) dir.delete()
        MMKV.initialize(dir.absolutePath, "88C51C536176AD8A8EE4A06F62EE897E")
    }
}