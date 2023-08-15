package net.yangkx.mmkv

import net.yangkx.mmkv.log.LogLevel
import net.yangkx.mmkv.log.Logger
import net.yangkx.mmkv.log.LoggerWrapper

object MMKV {
    init {
        System.loadLibrary("mmkv")
    }

    /***
     * Initialize the MMKV instance.
     *
     * All API calls before initialization will cause crash.
     *
     * Calling [initialize] multiple times is allowed,
     * the old instance will be closed (see [close]), the last call will take over.
     *
     * @param dir a writeable directory, absolute or relative paths are acceptable,
     * for example: `context.getDir("mmkv", Context.MODE_PRIVATE)`
     * @param key the key should be a hexadecimal string of length 16,
     * for example: `88C51C536176AD8A8EE4A06F62EE897E`
     */
    @JvmStatic
    external fun initialize(dir: String, key: String)

    @Throws(NativeException::class)
    external fun putString(key: String, value: String)

    @Throws(KeyNotFoundException::class)
    external fun getString(key: String): String

    fun getString(key: String, default: String): String {
        return try {
            getString(key)
        } catch (e: KeyNotFoundException) {
            default
        }
    }

    @Throws(NativeException::class)
    external fun putInt(key: String, value: Int)

    @Throws(KeyNotFoundException::class)
    external fun getInt(key: String): Int

    fun getInt(key: String, default: Int): Int {
        return try {
            getInt(key)
        } catch (e: KeyNotFoundException) {
            default
        }
    }

    @Throws(NativeException::class)
    external fun putBool(key: String, value: Boolean)

    @Throws(KeyNotFoundException::class)
    external fun getBool(key: String): Boolean

    fun getBool(key: String, default: Boolean): Boolean {
        return try {
            getBool(key)
        } catch (e: KeyNotFoundException) {
            default
        }
    }

    @Throws(NativeException::class)
    external fun putLong(key: String, value: Long)

    @Throws(KeyNotFoundException::class)
    external fun getLong(key: String): Long

    fun getLong(key: String, default: Long): Long {
        return try {
            getLong(key)
        } catch (e: KeyNotFoundException) {
            default
        }
    }

    @Throws(NativeException::class)
    external fun putFloat(key: String, value: Float)

    @Throws(KeyNotFoundException::class)
    external fun getFloat(key: String): Float

    fun getFloat(key: String, default: Float): Float {
        return try {
            getFloat(key)
        } catch (e: KeyNotFoundException) {
            default
        }
    }

    @Throws(NativeException::class)
    external fun putDouble(key: String, value: Double)

    @Throws(KeyNotFoundException::class)
    external fun getDouble(key: String): Double

    fun getDouble(key: String, default: Double): Double {
        return try {
            getDouble(key)
        } catch (e: KeyNotFoundException) {
            default
        }
    }

    @Throws(NativeException::class)
    external fun putByteArray(key: String, value: ByteArray)

    @Throws(KeyNotFoundException::class)
    external fun getByteArray(key: String): ByteArray

    fun getByteArray(key: String, default: ByteArray): ByteArray {
        return try {
            getByteArray(key)
        } catch (e: KeyNotFoundException) {
            default
        }
    }

    @Throws(NativeException::class)
    external fun putIntArray(key: String, value: IntArray)

    @Throws(KeyNotFoundException::class)
    external fun getIntArray(key: String): IntArray

    fun getIntArray(key: String, default: IntArray): IntArray {
        return try {
            getIntArray(key)
        } catch (e: KeyNotFoundException) {
            default
        }
    }

    @Throws(NativeException::class)
    external fun putLongArray(key: String, value: LongArray)

    @Throws(KeyNotFoundException::class)
    external fun getLongArray(key: String): LongArray

    fun getLongArray(key: String, default: LongArray): LongArray {
        return try {
            getLongArray(key)
        } catch (e: KeyNotFoundException) {
            default
        }
    }

    @Throws(NativeException::class)
    external fun putFloatArray(key: String, value: FloatArray)

    @Throws(KeyNotFoundException::class)
    external fun getFloatArray(key: String): FloatArray

    fun getFloatArray(key: String, default: FloatArray): FloatArray {
        return try {
            getFloatArray(key)
        } catch (e: KeyNotFoundException) {
            default
        }
    }

    @Throws(NativeException::class)
    external fun putDoubleArray(key: String, value: DoubleArray)

    @Throws(KeyNotFoundException::class)
    external fun getDoubleArray(key: String): DoubleArray

    fun getDoubleArray(key: String, default: DoubleArray): DoubleArray {
        return try {
            getDoubleArray(key)
        } catch (e: KeyNotFoundException) {
            default
        }
    }

    /**
     * Set log level to mmkv, the default log level is [LogLevel.VERBOSE],
     * logs with a larger level will be filtered out.
     */
    fun setLogLevel(level: LogLevel) {
        setLogLevel(level.ordinal)
    }

    private external fun setLogLevel(int: Int)

    /***
     * Clear all data and [close] the instance.
     * If you want to continue using the API, need to [initialize] again.
     */
    external fun clearData()

    /***
     * Close the instance to allow MMKV to initialize with different config.
     * If you want to continue using the API, need to [initialize] again.
     */
    external fun close()

    /**
     * Set a custom log implementation to MMKV, see [Logger]
     */
    fun setLogger(logger: Logger) {
        LoggerWrapper.setLogger(logger)
    }
}