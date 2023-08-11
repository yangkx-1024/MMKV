package net.yangkx.mmkv.log

import android.util.Log

/**
 * Implement this interface to redirect MMKV log out put.
 * See [LoggerWrapper.setLogger]
 */
interface Logger {
    fun verbose(log: String)
    fun info(log: String)
    fun debug(log: String)
    fun warn(log: String)
    fun error(log: String)
}

enum class LogLevel {
    CLOSE,
    ERROR,
    WARN,
    INFO,
    DEBUG,
    VERBOSE,
}

object LoggerWrapper {
    private const val TAG = "MMKV"

    private var logger: Logger = object : Logger {
        override fun verbose(log: String) {
            Log.v(TAG, log)
        }

        override fun info(log: String) {
            Log.i(TAG, log)
        }

        override fun debug(log: String) {
            Log.d(TAG, log)
        }

        override fun warn(log: String) {
            Log.w(TAG, log)
        }

        override fun error(log: String) {
            Log.e(TAG, log)
        }
    }

    fun setLogger(logger: Logger) {
        this.logger = logger
    }

    @JvmStatic
    fun verbose(log: String) {
        logger.verbose(log)
    }

    @JvmStatic
    fun info(log: String) {
        logger.info(log)
    }

    @JvmStatic
    fun debug(log: String) {
        logger.debug(log)
    }

    @JvmStatic
    fun warn(log: String) {
        logger.warn(log)
    }

    @JvmStatic
    fun error(log: String) {
        logger.error(log)
    }
}