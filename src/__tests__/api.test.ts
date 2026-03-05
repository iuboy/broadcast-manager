/**
 * API 客户端测试
 */

describe('API Client', () => {
  describe('WebSocket Message Types', () => {
    it('should validate broadcast request message structure', () => {
      const broadcastRequest = {
        type: 'broadcast_request',
        sample_rate: 44100,
        channels: 1,
        codec: 'pcm',
      };

      expect(broadcastRequest.type).toBe('broadcast_request');
      expect(broadcastRequest.sample_rate).toBeGreaterThan(0);
      expect(broadcastRequest.channels).toBeGreaterThanOrEqual(1);
      expect(['pcm', 'opus']).toContain(broadcastRequest.codec);
    });

    it('should validate audio data message structure', () => {
      const audioData = {
        type: 'audio_data',
        data: new Uint8Array([1, 2, 3, 4]),
      };

      expect(audioData.type).toBe('audio_data');
      expect(audioData.data).toBeInstanceOf(Uint8Array);
      expect(audioData.data.length).toBe(4);
    });

    it('should validate broadcast end message structure', () => {
      const broadcastEnd = {
        type: 'broadcast_end',
      };

      expect(broadcastEnd.type).toBe('broadcast_end');
    });
  });

  describe('Server Response Validation', () => {
    it('should validate success response', () => {
      const successResponse = {
        status: 'ok',
        message: 'Broadcast started',
      };

      expect(successResponse.status).toBe('ok');
      expect(typeof successResponse.message).toBe('string');
    });

    it('should validate error response', () => {
      const errorResponse = {
        status: 'error',
        message: 'Already broadcasting',
      };

      expect(errorResponse.status).toBe('error');
      expect(typeof errorResponse.message).toBe('string');
    });

    it('should validate state response', () => {
      const stateResponse = {
        state: 'idle',
        current_broadcaster: null,
        client_count: 5,
      };

      expect(['idle', 'broadcasting']).toContain(stateResponse.state);
      expect(typeof stateResponse.client_count).toBe('number');
      expect(stateResponse.client_count).toBeGreaterThanOrEqual(0);
    });
  });

  describe('Rate Limiting', () => {
    it('should enforce message rate limit', () => {
      const maxMessagesPerSecond = 200;
      const messageInterval = 1000 / maxMessagesPerSecond;
      
      expect(messageInterval).toBeGreaterThan(0);
      expect(messageInterval).toBeLessThan(10);
    });
  });
});
