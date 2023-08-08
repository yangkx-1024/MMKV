package net.yangkx.mmkv

import android.app.Application

class MyApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        MMKVInitializer.init(this)
    }
}