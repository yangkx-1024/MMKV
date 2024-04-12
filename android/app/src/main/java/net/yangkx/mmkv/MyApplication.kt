package net.yangkx.mmkv

import android.app.Application

class MyApplication : Application() {

    companion object {
        lateinit var mmkv: MMKV
    }

    override fun onCreate() {
        super.onCreate()
        mmkv = MMKVInitializer.init(this)
    }
}