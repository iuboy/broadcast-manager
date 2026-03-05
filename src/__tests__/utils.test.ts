/**
 * 工具函数单元测试
 */

describe('Utility Functions', () => {
  describe('Number Formatting', () => {
    it('should format volume percentage correctly', () => {
      const formatVolume = (value: number): string => {
        return Math.round(value * 100).toString();
      };

      expect(formatVolume(0.5)).toBe('50');
      expect(formatVolume(1.0)).toBe('100');
      expect(formatVolume(0)).toBe('0');
      expect(formatVolume(0.75)).toBe('75');
    });

    it('should parse volume percentage correctly', () => {
      const parseVolume = (value: string): number => {
        return parseInt(value, 10) / 100;
      };

      expect(parseVolume('50')).toBe(0.5);
      expect(parseVolume('100')).toBe(1.0);
      expect(parseVolume('0')).toBe(0);
    });
  });

  describe('Time Formatting', () => {
    it('should format seconds to MM:SS', () => {
      const formatTime = (seconds: number): string => {
        const mins = Math.floor(seconds / 60);
        const secs = Math.floor(seconds % 60);
        return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
      };

      expect(formatTime(0)).toBe('00:00');
      expect(formatTime(59)).toBe('00:59');
      expect(formatTime(60)).toBe('01:00');
      expect(formatTime(3661)).toBe('61:01');
    });
  });

  describe('File Path Utilities', () => {
    it('should extract filename from path', () => {
      const getFilename = (path: string): string => {
        // 使用 path.basename 风格的提取
        return path.replace(/^[\/\\]|[\/\\]$/g, '').split(/[\/\\]/).pop() || path;
      };

      expect(getFilename('/path/to/file.mp3')).toBe('file.mp3');
      expect(getFilename('C:/Music/song.wav')).toBe('song.wav');
      expect(getFilename('simple.flac')).toBe('simple.flac');
    });

    it('should extract file extension', () => {
      const getExtension = (filename: string): string => {
        const parts = filename.split('.');
        return parts.length > 1 ? parts.pop()!.toLowerCase() : '';
      };

      expect(getExtension('file.mp3')).toBe('mp3');
      expect(getExtension('file.WAV')).toBe('wav');
      expect(getExtension('noextension')).toBe('');
      expect(getExtension('archive.tar.gz')).toBe('gz');
    });
  });
});
