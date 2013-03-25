import datetime

from .utils import format_datetime


class SeriesDAO(object):
    def __init__(self, db, series_db, episodes_db, watched):
        self._db = db
        self._series_db = series_db
        self._episodes_db = episodes_db
        self._watched = watched

    def add(self, series):
        self._db.list_append("series", series['id'])
        self._series_db[str(series['id'])] = series

    def remove(self, series):
        self._db.list_remove("series", series['id'])
        del self._series_db[str(series['id'])]

    def has_series(self, series):
        return str(series['id']) in self._db.get("series", [])

    def list_series(self):
        result = list()

        for series_id in self._db.get("series", []):
            series = self._series_db.get(str(series_id))

            if not series:
                continue

            result.append(series)

        return result

    def find_series(self, series_query):
        result = list()

        series_query = series_query.lower()

        for series_id in self._db.get("series", []):
            series = self._series_db.get(str(series_id))

            if not series:
                continue

            if series_query not in series['series_name'].lower():
                continue

            result.append(series)

        return result

    def set_episodes(self, series, episodes):
        self._episodes_db[str(series['id'])] = episodes

    def get(self, series_id):
        return self._series_db.get(str(series_id))

    def get_episodes(self, series):
        return self._episodes_db.get(str(series['id']))

    def get_season_episodes(self, series, season_number):
        results = list()

        for episode in self._episodes_db.get(str(series['id'])):
            if episode['season_number'] != season_number:
                continue

            results.append(episode)

        return results

    def is_episode_watched(self, episode):
        return str(episode['id']) in self._watched

    def set_episode_watched(self, episode, watched=True):
        now = datetime.datetime.now()

        if watched:
            self._watched[str(episode['id'])] = format_datetime(now)
        else:
            del self._watched[str(episode['id'])]
