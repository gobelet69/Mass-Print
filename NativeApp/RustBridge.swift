import Foundation

@_silgen_name("mass_print_engine_new")
private func massPrintEngineNew() -> UnsafeMutableRawPointer?

@_silgen_name("mass_print_engine_call")
private func massPrintEngineCall(
    _ engine: UnsafeMutableRawPointer?,
    _ request: UnsafePointer<CChar>?
) -> UnsafeMutablePointer<CChar>?

@_silgen_name("mass_print_string_free")
private func massPrintStringFree(_ value: UnsafeMutablePointer<CChar>?)

@_silgen_name("mass_print_engine_free")
private func massPrintEngineFree(_ engine: UnsafeMutableRawPointer?)

private struct BridgeResponse<Value: Decodable>: Decodable {
    let ok: Bool
    let data: Value?
    let error: String?
}

enum RustBridgeError: LocalizedError {
    case unavailable
    case invalidResponse
    case engine(String)

    var errorDescription: String? {
        switch self {
        case .unavailable: return "The native Rust engine is unavailable."
        case .invalidResponse: return "The native Rust engine returned an invalid response."
        case .engine(let message): return message
        }
    }
}

final class RustEngine: @unchecked Sendable {
    private let handle: UnsafeMutableRawPointer
    private let encoder = JSONSerialization.self

    init() throws {
        guard let handle = massPrintEngineNew() else {
            throw RustBridgeError.unavailable
        }
        self.handle = handle
    }

    deinit {
        massPrintEngineFree(handle)
    }

    func call<Value: Decodable>(_ request: [String: Any], as: Value.Type = Value.self) throws -> Value {
        let requestData = try encoder.data(withJSONObject: request)
        guard let requestString = String(data: requestData, encoding: .utf8) else {
            throw RustBridgeError.invalidResponse
        }

        let responsePointer = requestString.withCString { pointer in
            massPrintEngineCall(handle, pointer)
        }
        guard let responsePointer else {
            throw RustBridgeError.invalidResponse
        }
        defer { massPrintStringFree(responsePointer) }

        let responseData = Data(String(cString: responsePointer).utf8)
        let response = try JSONDecoder().decode(BridgeResponse<Value>.self, from: responseData)
        if response.ok, let data = response.data {
            return data
        }
        throw RustBridgeError.engine(response.error ?? "Unknown native engine error")
    }
}
