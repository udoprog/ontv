import datetime

from .utils import format_datetime


class SeriesDAO(object):
    def __init__(self, db_series, series, episodes, watched):
        self._db_series = db_series
        self._series = series
        self._episodes = episodes
        self._watched = watched

    def add(self, series):
        self._db_series.add(series['id'])
        self._series[series['id']] = series

    def remove(self, series):
        self._db_series.remove(series['id'])
        del self._series[series['id']]

    def has_series(self, series):
        return series['id'] in self._db_series

    def list_series(self):
        result = list()

        for series_id in self._db_series:
            series = self._series.get(series_id)

            if not series:
                continue

            result.append(series)

        return result

    def find_series(self, series_query):
        result = list()

        series_query = series_query.lower()

        for series_id in self._db_series:
            series = self._series.get(series_id)

            if not series:
                continue

            if series_query not in series['series_name'].lower():
                continue

            result.append(series)

        return result

    def set_episodes(self, series, episodes):
        self._episodes[series['id']] = episodes

    def get(self, series_id):
        return self._series.get(series_id)

    def get_episodes(self, series):
        return self._episodes.get(series['id'])

    def get_season_episodes(self, series, season_number):
        results = list()

        for episode in self._episodes.get(series['id']):
            if episode['season_number'] != season_number:
                continue

            results.append(episode)

        return results

    def is_episode_watched(self, episode):
        return episode['id'] in self._watched

    def set_episode_watched(self, episode, watched=True):
        now = datetime.datetime.now()

        if watched:
            self._watched[episode['id']] = format_datetime(now)
        else:
            del self._watched[episode['id']]
